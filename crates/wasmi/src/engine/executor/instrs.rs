pub use self::call::CallKind;
use self::{call::CallOutcome, return_::ReturnOutcome};
use crate::{
    core::{TrapCode, UntypedValue},
    engine::{
        bytecode::{
            AnyConst32, BinInstr, BinInstrImm16, BlockFuel, Const16, FuncIdx, Instruction,
            Register, RegisterSpan, UnaryInstr,
        },
        cache::InstanceCache,
        code_map::InstructionPtr,
        executor::stack::{CallFrame, CallStack, FrameRegisters, ValueStack},
        func_types::FuncTypeRegistry,
        CodeMap,
    },
    store::ResourceLimiterRef,
    Error, Func, FuncRef, StoreInner,
};

mod binary;
mod branch;
mod call;
mod comparison;
mod conversion;
mod copy;
mod global;
mod load;
mod memory;
mod return_;
mod select;
mod store;
mod table;
mod unary;

macro_rules! forward_call {
    ($expr:expr) => {{
        if let CallOutcome::Call {
            results,
            host_func,
            call_kind,
        } = $expr?
        {
            return Ok(WasmOutcome::Call {
                results,
                host_func,
                call_kind,
            });
        }
    }};
}

macro_rules! forward_return {
    ($expr:expr) => {{
        if let ReturnOutcome::Host = $expr {
            return Ok(WasmOutcome::Return);
        }
    }};
}

/// The outcome of a Wasm execution.
///
/// # Note
///
/// A Wasm execution includes everything but host calls.
/// In other words: Everything in between host calls is a Wasm execution.
#[derive(Debug, Copy, Clone)]
pub enum WasmOutcome {
    /// The Wasm execution has ended and returns to the host side.
    Return,
    /// The Wasm execution calls a host function.
    Call {
        results: RegisterSpan,
        host_func: Func,
        call_kind: CallKind,
    },
}

/// Executes compiled function instructions until either
///
/// - returning from the root function
/// - calling a host function
/// - encountering a trap
///
/// # Errors
///
/// If the execution traps.
#[inline(never)]
pub fn execute_instrs<'ctx, 'engine>(
    ctx: &'ctx mut StoreInner,
    cache: &'engine mut InstanceCache,
    value_stack: &'engine mut ValueStack,
    call_stack: &'engine mut CallStack,
    code_map: &'engine CodeMap,
    func_types: &'engine FuncTypeRegistry,
    resource_limiter: &'ctx mut ResourceLimiterRef<'ctx>,
) -> Result<WasmOutcome, Error> {
    Executor::new(ctx, cache, value_stack, call_stack, code_map, func_types)
        .execute(resource_limiter)
}

/// An execution context for executing a Wasmi function frame.
#[derive(Debug)]
struct Executor<'ctx, 'engine> {
    /// Stores the value stack of live values on the Wasm stack.
    sp: FrameRegisters,
    /// The pointer to the currently executed instruction.
    ip: InstructionPtr,
    /// Stores frequently used instance related data.
    cache: &'engine mut InstanceCache,
    /// A mutable [`StoreInner`] context.
    ///
    /// [`StoreInner`]: [`crate::StoreInner`]
    ctx: &'ctx mut StoreInner,
    /// The value stack.
    ///
    /// # Note
    ///
    /// This reference is mainly used to synchronize back state
    /// after manipulations to the value stack via `sp`.
    value_stack: &'engine mut ValueStack,
    /// The call stack.
    ///
    /// # Note
    ///
    /// This is used to store the stack of nested function calls.
    call_stack: &'engine mut CallStack,
    /// The Wasm function code map.
    ///
    /// # Note
    ///
    /// This is used to lookup Wasm function information.
    code_map: &'engine CodeMap,
    /// The Wasm function type registry.
    ///
    /// # Note
    ///
    /// This is used to lookup Wasm function information.
    func_types: &'engine FuncTypeRegistry,
}

impl<'ctx, 'engine> Executor<'ctx, 'engine> {
    /// Creates a new [`Executor`] for executing a Wasmi function frame.
    #[inline(always)]
    pub fn new(
        ctx: &'ctx mut StoreInner,
        cache: &'engine mut InstanceCache,
        value_stack: &'engine mut ValueStack,
        call_stack: &'engine mut CallStack,
        code_map: &'engine CodeMap,
        func_types: &'engine FuncTypeRegistry,
    ) -> Self {
        let frame = call_stack
            .peek()
            .expect("must have call frame on the call stack");
        // Safety: We are using the frame's own base offset as input because it is
        //         guaranteed by the Wasm validation and translation phase to be
        //         valid for all register indices used by the associated function body.
        let sp = unsafe { value_stack.stack_ptr_at(frame.base_offset()) };
        let ip = frame.instr_ptr();
        Self {
            sp,
            ip,
            cache,
            ctx,
            value_stack,
            call_stack,
            code_map,
            func_types,
        }
    }

    /// Executes the function frame until it returns or traps.
    #[inline(always)]
    fn execute(
        mut self,
        resource_limiter: &'ctx mut ResourceLimiterRef<'ctx>,
    ) -> Result<WasmOutcome, Error> {
        use Instruction as Instr;
        loop {
            let instr = *self.ip.get();
            if self.ctx.engine().config().get_update_runtime_signature() {
                // update the runtime signature with the current instruction
                // we map the instruction to a unique 64-bit prime number
                let instr_prime = match instr {
                    Instr::TableIdx(_) => 0xf360371a61b48ca1,
                    Instr::DataSegmentIdx(_) => 0xce5750f577a4a9bd,
                    Instr::ElementSegmentIdx(_) => 0xdb013c4da009cbe9,
                    Instr::Const32(_) => 0xe3a461c24c1edf67,
                    Instr::I64Const32(_) => 0x93e0632ef59fbf8d,
                    Instr::F64Const32(_) => 0xcf96777f6bf48827,
                    Instr::Register(_) => 0xa1a9bcb9fec5fdfb,
                    Instr::Register2(_) => 0xbee08b06e6ab17f5,
                    Instr::Register3(_) => 0xb448b4a7d84f751f,
                    Instr::RegisterList(_) => 0xb918e0472d8c224f,
                    Instr::CallIndirectParams(_) => 0xbf382b4acfe7644b,
                    Instr::CallIndirectParamsImm16(_) => 0xd853e6a184c25f0d,
                    Instr::Trap(_) => 0xb18d650b9f5998a7,
                    Instr::ConsumeFuel(_) => 0xe6118441cda42713,
                    Instr::Return => 0xc8b8b1c1bcbd90e5,
                    Instr::ReturnReg { .. } => 0xbaab8e9341e08dbf,
                    Instr::ReturnReg2 { .. } => 0xa73d1157b48ca275,
                    Instr::ReturnReg3 { .. } => 0xa6ffa6328ec1eb81,
                    Instr::ReturnImm32 { .. } => 0x83307e10c33705a3,
                    Instr::ReturnI64Imm32 { .. } => 0xe8a6034d312d2135,
                    Instr::ReturnF64Imm32 { .. } => 0xdc28177292727dc9,
                    Instr::ReturnSpan { .. } => 0xda75d553c9a0933b,
                    Instr::ReturnMany { .. } => 0xa5084225f2090f95,
                    Instr::ReturnNez { .. } => 0xe31a3fb6dd7f310d,
                    Instr::ReturnNezReg { .. } => 0xbfd53817fb0381e7,
                    Instr::ReturnNezReg2 { .. } => 0xd85809e11f54d745,
                    Instr::ReturnNezImm32 { .. } => 0xddb81fb1a74a83b1,
                    Instr::ReturnNezI64Imm32 { .. } => 0x860254a7c93ec93f,
                    Instr::ReturnNezF64Imm32 { .. } => 0xb2ee1c9ad5f914bb,
                    Instr::ReturnNezSpan { .. } => 0xec3158e4f69f44df,
                    Instr::ReturnNezMany { .. } => 0xc6cdd0d8f17fe649,
                    Instr::Branch { .. } => 0xef66bf425478625b,
                    Instr::BranchCmpFallback { .. } => 0x87d943ccc553c97f,
                    Instr::BranchI32And(_) => 0xf16d67d2a7dbc15b,
                    Instr::BranchI32AndImm(_) => 0xd97e76e4a08a4169,
                    Instr::BranchI32Or(_) => 0xac6e6dcc9eb6cbff,
                    Instr::BranchI32OrImm(_) => 0xa36564ae5f8bcf13,
                    Instr::BranchI32Xor(_) => 0xa3fb8b494d435729,
                    Instr::BranchI32XorImm(_) => 0xd8a580b0d15cf0ab,
                    Instr::BranchI32AndEqz(_) => 0xc118754f6fd4adc1,
                    Instr::BranchI32AndEqzImm(_) => 0xa90fbb32f7b47dc7,
                    Instr::BranchI32OrEqz(_) => 0xa1bf533d0d3f0635,
                    Instr::BranchI32OrEqzImm(_) => 0xfe99000769fe6ddd,
                    Instr::BranchI32XorEqz(_) => 0xe2ade8751fc2e9a3,
                    Instr::BranchI32XorEqzImm(_) => 0xc2c831b19dd7b0d3,
                    Instr::BranchI32Eq(_) => 0xa9504bf5d4a47f69,
                    Instr::BranchI32EqImm(_) => 0xcc68c4fcdd5df33b,
                    Instr::BranchI32Ne(_) => 0xc574d8a05da369d3,
                    Instr::BranchI32NeImm(_) => 0xcad08b87db831f77,
                    Instr::BranchI32LtS(_) => 0xc590acad04f1f7b9,
                    Instr::BranchI32LtSImm(_) => 0xd4d918a2cfb5323d,
                    Instr::BranchI32LtU(_) => 0xc4999a7e79065d73,
                    Instr::BranchI32LtUImm(_) => 0xf4fbdab953a405df,
                    Instr::BranchI32LeS(_) => 0x98a04abe0fa4ce01,
                    Instr::BranchI32LeSImm(_) => 0xa756dc299bd21ea7,
                    Instr::BranchI32LeU(_) => 0xebe5a83153067f95,
                    Instr::BranchI32LeUImm(_) => 0xd6adc84185c3b835,
                    Instr::BranchI32GtS(_) => 0xc77aef230f5cb5c1,
                    Instr::BranchI32GtSImm(_) => 0xb288abe58caf78fd,
                    Instr::BranchI32GtU(_) => 0xdd85783639dea14b,
                    Instr::BranchI32GtUImm(_) => 0xc95d435e3bd01389,
                    Instr::BranchI32GeS(_) => 0xe448369b7242bd3b,
                    Instr::BranchI32GeSImm(_) => 0xd3ed1490c07aec79,
                    Instr::BranchI32GeU(_) => 0xcbdfc0da7497aca9,
                    Instr::BranchI32GeUImm(_) => 0xd01255cca5331a55,
                    Instr::BranchI64Eq(_) => 0xd224c9cfe6c84099,
                    Instr::BranchI64EqImm(_) => 0xb1f5e1ce9cb796ed,
                    Instr::BranchI64Ne(_) => 0xa015db66e4480f37,
                    Instr::BranchI64NeImm(_) => 0xc9534063141f1b6d,
                    Instr::BranchI64LtS(_) => 0xf3c68e18c0fc1c3b,
                    Instr::BranchI64LtSImm(_) => 0xfaadf3a5cd945423,
                    Instr::BranchI64LtU(_) => 0xe12e4e46df02fc2f,
                    Instr::BranchI64LtUImm(_) => 0xb3476ce898e10f3d,
                    Instr::BranchI64LeS(_) => 0xfb1cbfc1097a9473,
                    Instr::BranchI64LeSImm(_) => 0xb4167d6222fadaf7,
                    Instr::BranchI64LeU(_) => 0xb2932efcea953cab,
                    Instr::BranchI64LeUImm(_) => 0x821f8f708d1f974f,
                    Instr::BranchI64GtS(_) => 0xde5463b08e9f4729,
                    Instr::BranchI64GtSImm(_) => 0xd765407968c91f01,
                    Instr::BranchI64GtU(_) => 0xe2c63c2c0678900b,
                    Instr::BranchI64GtUImm(_) => 0xd035ff821066bb9d,
                    Instr::BranchI64GeS(_) => 0xe49707e335868fa5,
                    Instr::BranchI64GeSImm(_) => 0xf857874dc48a27e9,
                    Instr::BranchI64GeU(_) => 0x8b3ce0fa63214359,
                    Instr::BranchI64GeUImm(_) => 0x93f90f4418d24385,
                    Instr::BranchF32Eq(_) => 0x8647b33a7b8d4ea9,
                    Instr::BranchF32Ne(_) => 0x9efcbece1096b201,
                    Instr::BranchF32Lt(_) => 0xb2ab8327611d4843,
                    Instr::BranchF32Le(_) => 0xfdb94010ae03ebad,
                    Instr::BranchF32Gt(_) => 0xc74489c6752ef2e3,
                    Instr::BranchF32Ge(_) => 0xb2588add33b6dc8d,
                    Instr::BranchF64Eq(_) => 0xb0f911188eef530b,
                    Instr::BranchF64Ne(_) => 0xb3a436328722e3af,
                    Instr::BranchF64Lt(_) => 0x996ae1e7999d71a5,
                    Instr::BranchF64Le(_) => 0xb00795c450f79fd7,
                    Instr::BranchF64Gt(_) => 0xfd0f65f70976783f,
                    Instr::BranchF64Ge(_) => 0xab728f867409f623,
                    Instr::BranchTable { .. } => 0xe2510e47b282102d,
                    Instr::Copy { .. } => 0xf476618f2886dc2f,
                    Instr::Copy2 { .. } => 0x81e0ef8904c1cfd5,
                    Instr::CopyImm32 { .. } => 0xaafc797a3f40deeb,
                    Instr::CopyI64Imm32 { .. } => 0xe6dddd163140692f,
                    Instr::CopyF64Imm32 { .. } => 0x976bb2d6ce6f3ccf,
                    Instr::CopySpan { .. } => 0x84f01169f85f4fff,
                    Instr::CopySpanNonOverlapping { .. } => 0xb32e9b533c4e6e29,
                    Instr::CopyMany { .. } => 0xe63c8f65639ebb8f,
                    Instr::CopyManyNonOverlapping { .. } => 0xeeb9a195160ac2d7,
                    Instr::ReturnCallInternal0 { .. } => 0xec2eb6bd5a5fc313,
                    Instr::ReturnCallInternal { .. } => 0xb833133b9b99a663,
                    Instr::ReturnCallImported0 { .. } => 0xe14f7c46f5f83b6b,
                    Instr::ReturnCallImported { .. } => 0x84e5c91bee77ebb7,
                    Instr::ReturnCallIndirect0 { .. } => 0xc5aadca828024d75,
                    Instr::ReturnCallIndirect { .. } => 0x814db79997981ca9,
                    Instr::CallInternal0 { .. } => 0xfbb357448a8642c3,
                    Instr::CallInternal { .. } => 0xab093cfe38b97547,
                    Instr::CallImported0 { .. } => 0xe866c937356994c5,
                    Instr::CallImported { .. } => 0xa9b3f7092e7cd01b,
                    Instr::CallIndirect0 { .. } => 0x89fdcc51af24bead,
                    Instr::CallIndirect { .. } => 0xbda3e8601077a917,
                    Instr::Select { .. } => 0xcab5aefcb578755f,
                    Instr::SelectRev { .. } => 0xf0a2df16fbbb44ff,
                    Instr::SelectImm32 { .. } => 0xe640723b1c13c87f,
                    Instr::SelectI64Imm32 { .. } => 0xdcdfa8f4a8043ef7,
                    Instr::SelectF64Imm32 { .. } => 0x9bbf27a9403e07e3,
                    Instr::RefFunc { .. } => 0xd1cd7a96bb99ad23,
                    Instr::TableGet { .. } => 0x90f6c6bb3c114319,
                    Instr::TableGetImm { .. } => 0x9595b2107e23cb21,
                    Instr::TableSize { .. } => 0xd396ced918e61bc5,
                    Instr::TableSet { .. } => 0xf5b649b2b404d197,
                    Instr::TableSetAt { .. } => 0xcdebf347b50872d3,
                    Instr::TableCopy { .. } => 0xf422dd12f6642265,
                    Instr::TableCopyTo { .. } => 0xfe2f83b88da7fa03,
                    Instr::TableCopyFrom { .. } => 0xfe202c3d504679e1,
                    Instr::TableCopyFromTo { .. } => 0x9d9ebedd147ee0c3,
                    Instr::TableCopyExact { .. } => 0x9dcc8c066927a9db,
                    Instr::TableCopyToExact { .. } => 0xbe7fae07ca7d32ef,
                    Instr::TableCopyFromExact { .. } => 0x88d7ecf054f2807d,
                    Instr::TableCopyFromToExact { .. } => 0xff641f66fa9a63d3,
                    Instr::TableInit { .. } => 0x82bb6ff383050763,
                    Instr::TableInitTo { .. } => 0x932d2a71ef983f85,
                    Instr::TableInitFrom { .. } => 0xd8cdfe120accedcf,
                    Instr::TableInitFromTo { .. } => 0xebca0cc0890416c5,
                    Instr::TableInitExact { .. } => 0xc626dd2b280ae6e3,
                    Instr::TableInitToExact { .. } => 0xdace86517a593a71,
                    Instr::TableInitFromExact { .. } => 0xe5c82cb9a1eac895,
                    Instr::TableInitFromToExact { .. } => 0xe5450bc5eb2d7631,
                    Instr::TableFill { .. } => 0xe4e6730feefdc50f,
                    Instr::TableFillAt { .. } => 0xc23bd8546b888c6b,
                    Instr::TableFillExact { .. } => 0x98f8babdc99204f3,
                    Instr::TableFillAtExact { .. } => 0x8e49dd000adf0689,
                    Instr::TableGrow { .. } => 0x9e61c8c958c6b891,
                    Instr::TableGrowImm { .. } => 0x927de647f4278045,
                    Instr::ElemDrop(_) => 0xbc4deb8b398e8a67,
                    Instr::DataDrop(_) => 0xaf73214c7ebdae49,
                    Instr::MemorySize { .. } => 0xc99e9ec6fd30df43,
                    Instr::MemoryGrow { .. } => 0x902226df112aa763,
                    Instr::MemoryGrowBy { .. } => 0xded192652730b3f3,
                    Instr::MemoryCopy { .. } => 0xf84118c356d104eb,
                    Instr::MemoryCopyTo { .. } => 0xf734ad61d9950a83,
                    Instr::MemoryCopyFrom { .. } => 0x9a5467d705c1581d,
                    Instr::MemoryCopyFromTo { .. } => 0xc087075594f5ef01,
                    Instr::MemoryCopyExact { .. } => 0xa300dfa09a5404db,
                    Instr::MemoryCopyToExact { .. } => 0xdecaec4c2f332687,
                    Instr::MemoryCopyFromExact { .. } => 0xd973b50e06e7190d,
                    Instr::MemoryCopyFromToExact { .. } => 0xd09c8ab6e9e82db3,
                    Instr::MemoryFill { .. } => 0x853e90844bc5b9c1,
                    Instr::MemoryFillAt { .. } => 0xba3e3f0c5214daf9,
                    Instr::MemoryFillImm { .. } => 0xebb50579d7dc30cb,
                    Instr::MemoryFillExact { .. } => 0x82bf6b083bb7ab07,
                    Instr::MemoryFillAtImm { .. } => 0xb721067ce69b7335,
                    Instr::MemoryFillAtExact { .. } => 0xafa4befd04378e05,
                    Instr::MemoryFillImmExact { .. } => 0x81e2eca79e7e30c1,
                    Instr::MemoryFillAtImmExact { .. } => 0xf4c5f89614b61c1b,
                    Instr::MemoryInit { .. } => 0xa30ffa4319eca43b,
                    Instr::MemoryInitTo { .. } => 0x8bc41f3bd9e53945,
                    Instr::MemoryInitFrom { .. } => 0x922c8e0448c76ad1,
                    Instr::MemoryInitFromTo { .. } => 0xdc74c7e86a7d9527,
                    Instr::MemoryInitExact { .. } => 0xe8e1aabd32798baf,
                    Instr::MemoryInitToExact { .. } => 0xa4612593e32593ab,
                    Instr::MemoryInitFromExact { .. } => 0xe6576301838fe52f,
                    Instr::MemoryInitFromToExact { .. } => 0xb2b93bf089ba4b27,
                    Instr::GlobalGet { .. } => 0x8d923111b80b5901,
                    Instr::GlobalSet { .. } => 0xe498f909f87cf3d7,
                    Instr::GlobalSetI32Imm16 { .. } => 0xbeceb62a094167cf,
                    Instr::GlobalSetI64Imm16 { .. } => 0xb255daab1ca25487,
                    Instr::I32Load(_) => 0xdf5b9b6fa80f3631,
                    Instr::I32LoadAt(_) => 0xf78ad97d27554aab,
                    Instr::I32LoadOffset16(_) => 0x8d191c3c9f983b7d,
                    Instr::I64Load(_) => 0xcde7973deae4d139,
                    Instr::I64LoadAt(_) => 0xc07cc699947471df,
                    Instr::I64LoadOffset16(_) => 0xbfd2b00e2b3c39d5,
                    Instr::F32Load(_) => 0xef1fbab218f04407,
                    Instr::F32LoadAt(_) => 0xa8306192cd73002d,
                    Instr::F32LoadOffset16(_) => 0xed0992f6c6239c7f,
                    Instr::F64Load(_) => 0xf6689ac5b352c02f,
                    Instr::F64LoadAt(_) => 0x97f205959c2a3d0b,
                    Instr::F64LoadOffset16(_) => 0x94fbb4628a79462b,
                    Instr::I32Load8s(_) => 0xfbb04e5f0a302d7b,
                    Instr::I32Load8sAt(_) => 0x8e95f3bd70e298e7,
                    Instr::I32Load8sOffset16(_) => 0xb736c7c8935178f5,
                    Instr::I32Load8u(_) => 0xf0e219ca1d327f63,
                    Instr::I32Load8uAt(_) => 0xc5ca3a6dc78a1a5d,
                    Instr::I32Load8uOffset16(_) => 0xc1932ac6c5cd54ff,
                    Instr::I32Load16s(_) => 0xe74c775c66d1dac7,
                    Instr::I32Load16sAt(_) => 0xbc3c7a6541752f39,
                    Instr::I32Load16sOffset16(_) => 0x98c1f9f35f8f6c6f,
                    Instr::I32Load16u(_) => 0xdc6866c6770da481,
                    Instr::I32Load16uAt(_) => 0xf194f68751968d29,
                    Instr::I32Load16uOffset16(_) => 0xfc6373feac795559,
                    Instr::I64Load8s(_) => 0xe727f7f48695f6ad,
                    Instr::I64Load8sAt(_) => 0x9fccd4f7bd3f283f,
                    Instr::I64Load8sOffset16(_) => 0xe865fdf1a1c55585,
                    Instr::I64Load8u(_) => 0xf78018cfa4de9cf9,
                    Instr::I64Load8uAt(_) => 0xed4846b1ee465189,
                    Instr::I64Load8uOffset16(_) => 0xeb9c4fdbd7a69a7d,
                    Instr::I64Load16s(_) => 0xce757e747c1781e1,
                    Instr::I64Load16sAt(_) => 0x8f96d62fc6381b5b,
                    Instr::I64Load16sOffset16(_) => 0x81747c9166be968d,
                    Instr::I64Load16u(_) => 0x9d169d9c81872e09,
                    Instr::I64Load16uAt(_) => 0x9ff242a4f7087a3b,
                    Instr::I64Load16uOffset16(_) => 0x8a58890d2d2e95fd,
                    Instr::I64Load32s(_) => 0xc7b0ed9c7dd80abb,
                    Instr::I64Load32sAt(_) => 0xd22e5e85c5df8b81,
                    Instr::I64Load32sOffset16(_) => 0xfe197c431899c773,
                    Instr::I64Load32u(_) => 0xec214adc8d89b335,
                    Instr::I64Load32uAt(_) => 0x8546452698268a41,
                    Instr::I64Load32uOffset16(_) => 0x900023566f7219db,
                    Instr::I32Store(_) => 0x89b4696626e6200f,
                    Instr::I32StoreOffset16(_) => 0xa9624220aa646c45,
                    Instr::I32StoreOffset16Imm16(_) => 0xd375c7c6e96da7eb,
                    Instr::I32StoreAt(_) => 0x9507335cdf40a30f,
                    Instr::I32StoreAtImm16(_) => 0xb124dcb1efb5a56f,
                    Instr::I32Store8(_) => 0xb40f3d40e5cbc63f,
                    Instr::I32Store8Offset16(_) => 0xb7784c5f610fa6b9,
                    Instr::I32Store8Offset16Imm(_) => 0xb1b94e6edc784d75,
                    Instr::I32Store8At(_) => 0xc114292b9396fca1,
                    Instr::I32Store8AtImm(_) => 0xf958f99a724d3fa9,
                    Instr::I32Store16(_) => 0xf4db4b8b777ba485,
                    Instr::I32Store16Offset16(_) => 0x917265d951560b9f,
                    Instr::I32Store16Offset16Imm(_) => 0x8e59e4b976ddd5c9,
                    Instr::I32Store16At(_) => 0x85e34459fca92a63,
                    Instr::I32Store16AtImm(_) => 0xffca87a7a28dcaaf,
                    Instr::I64Store(_) => 0xaa0cfbf1401da505,
                    Instr::I64StoreOffset16(_) => 0xde11b832af36e2c3,
                    Instr::I64StoreOffset16Imm16(_) => 0x93a03ca4c630054d,
                    Instr::I64StoreAt(_) => 0x8b7be36a892dbe9f,
                    Instr::I64StoreAtImm16(_) => 0xa7164db75f5ffc79,
                    Instr::I64Store8(_) => 0xb16fc3bd7fcf8229,
                    Instr::I64Store8Offset16(_) => 0xf5324129bf7f4299,
                    Instr::I64Store8Offset16Imm(_) => 0xeb1df0108fb325c1,
                    Instr::I64Store8At(_) => 0xcc72df888ac47c3f,
                    Instr::I64Store8AtImm(_) => 0x90e2c84d2be4491b,
                    Instr::I64Store16(_) => 0xa670b61daad1097f,
                    Instr::I64Store16Offset16(_) => 0xd09b793649e2dc69,
                    Instr::I64Store16Offset16Imm(_) => 0xc5733c19fee00329,
                    Instr::I64Store16At(_) => 0xc3471d0e7d859cdd,
                    Instr::I64Store16AtImm(_) => 0xa27e4cfa22b0d101,
                    Instr::I64Store32(_) => 0xacade9332186dab9,
                    Instr::I64Store32Offset16(_) => 0xb5777e453e6429dd,
                    Instr::I64Store32Offset16Imm16(_) => 0xc8435df9b5285e43,
                    Instr::I64Store32At(_) => 0xb1cb0f6ea058bbbb,
                    Instr::I64Store32AtImm16(_) => 0x8f79394f20bbda89,
                    Instr::F32Store(_) => 0xd6df58b0ab76e99f,
                    Instr::F32StoreOffset16(_) => 0xff1461bc14215f77,
                    Instr::F32StoreAt(_) => 0xd2f62bd6fa3c90b9,
                    Instr::F64Store(_) => 0xda484e6b7bd8d5db,
                    Instr::F64StoreOffset16(_) => 0xac6256a3ca2605cb,
                    Instr::F64StoreAt(_) => 0xe366beba3742040b,
                    Instr::I32Eq(_) => 0x9aa2499f95dc3711,
                    Instr::I32EqImm16(_) => 0x92ce6da978fdc40f,
                    Instr::I64Eq(_) => 0xda860a17cb3b1a8b,
                    Instr::I64EqImm16(_) => 0x89c423624314bf89,
                    Instr::I32Ne(_) => 0xcbad0daca146769f,
                    Instr::I32NeImm16(_) => 0xdca831c7fde0f85f,
                    Instr::I64Ne(_) => 0xb0b2865912833697,
                    Instr::I64NeImm16(_) => 0xbafcb515ea4df971,
                    Instr::I32LtS(_) => 0xbfbff0c826a235f1,
                    Instr::I32LtU(_) => 0x9081189b58e72897,
                    Instr::I32LtSImm16(_) => 0x96d3c1dc900e1187,
                    Instr::I32LtUImm16(_) => 0x9025ddff9d4de1b9,
                    Instr::I64LtS(_) => 0xffe594b2eb58493d,
                    Instr::I64LtU(_) => 0x9181211dcc10809b,
                    Instr::I64LtSImm16(_) => 0xebbe15881dcd9e57,
                    Instr::I64LtUImm16(_) => 0xfd542ad322324a27,
                    Instr::I32GtS(_) => 0xe36a0bdacb5debf3,
                    Instr::I32GtU(_) => 0xa5deeacd3be1c44b,
                    Instr::I32GtSImm16(_) => 0x9a7f06186b3795c3,
                    Instr::I32GtUImm16(_) => 0xaaf1e009bd26fd7b,
                    Instr::I64GtS(_) => 0xc548cf95ce91a7b5,
                    Instr::I64GtU(_) => 0xa68907e0fab9fb93,
                    Instr::I64GtSImm16(_) => 0xf62c848e8eef17e9,
                    Instr::I64GtUImm16(_) => 0x8a5c73b85ba194e1,
                    Instr::I32LeS(_) => 0xbc5f6381dd176bb1,
                    Instr::I32LeU(_) => 0xf7f780c2e00e18af,
                    Instr::I32LeSImm16(_) => 0xc79fde79bd40aa77,
                    Instr::I32LeUImm16(_) => 0xf9cc6a0ad9269149,
                    Instr::I64LeS(_) => 0xc7fac5fcb8f8ed55,
                    Instr::I64LeU(_) => 0xc1560dd513285d29,
                    Instr::I64LeSImm16(_) => 0x99c89d3bbb73545d,
                    Instr::I64LeUImm16(_) => 0xa3524cb19ca06bb5,
                    Instr::I32GeS(_) => 0x98bef5ced76c7645,
                    Instr::I32GeU(_) => 0xd53ed9432d2a8143,
                    Instr::I32GeSImm16(_) => 0xfee355988dee53db,
                    Instr::I32GeUImm16(_) => 0xbb62bd7c6348e5c3,
                    Instr::I64GeS(_) => 0xd8f40b0c313c453d,
                    Instr::I64GeU(_) => 0x98bfaac3f19897e1,
                    Instr::I64GeSImm16(_) => 0x8956eaaa98c2e647,
                    Instr::I64GeUImm16(_) => 0xe11e8b930ba0afed,
                    Instr::F32Eq(_) => 0xc3587b028ec7b7d7,
                    Instr::F64Eq(_) => 0x90fa962604933679,
                    Instr::F32Ne(_) => 0xe8b028b8a40b6323,
                    Instr::F64Ne(_) => 0xe511c632ed75d0ad,
                    Instr::F32Lt(_) => 0xafe8adc3497d922f,
                    Instr::F64Lt(_) => 0xa24d51fe3b08563d,
                    Instr::F32Le(_) => 0xa9470d623de1df2f,
                    Instr::F64Le(_) => 0xe5e889a7f2d74d67,
                    Instr::F32Gt(_) => 0x8cbe5aa7efd2dac5,
                    Instr::F64Gt(_) => 0xa2b5a501d74cc69b,
                    Instr::F32Ge(_) => 0x9103bfb43045fc5b,
                    Instr::F64Ge(_) => 0xe4832a9c5a4a0741,
                    Instr::I32Clz(_) => 0xd0b363eee33e2a75,
                    Instr::I64Clz(_) => 0xbb13e80b90e6d539,
                    Instr::I32Ctz(_) => 0xa867670e58678389,
                    Instr::I64Ctz(_) => 0x8ad83f5db31d4957,
                    Instr::I32Popcnt(_) => 0xd8ad8c4a45f7cd09,
                    Instr::I64Popcnt(_) => 0xb9603f856bc14e5b,
                    Instr::I32Add(_) => 0xa1f888c0cefc7b6d,
                    Instr::I64Add(_) => 0xba1adc988a80490f,
                    Instr::I32AddImm16(_) => 0x8bc5e0c56da6ee3d,
                    Instr::I64AddImm16(_) => 0xf75f1d741e812869,
                    Instr::I32Sub(_) => 0xf095a662a345025f,
                    Instr::I64Sub(_) => 0xb251e2585fd105c7,
                    Instr::I32SubImm16(_) => 0xfbca30bbb6a376e7,
                    Instr::I32SubImm16Rev(_) => 0x9bccf96d076e1e77,
                    Instr::I64SubImm16(_) => 0x863549d6231f46d5,
                    Instr::I64SubImm16Rev(_) => 0xf65f7135553a54d5,
                    Instr::I32Mul(_) => 0xc36cfc74fa61f2b3,
                    Instr::I64Mul(_) => 0xe9fe8ad570b71a99,
                    Instr::I32MulImm16(_) => 0x99c04a0680397c59,
                    Instr::I64MulImm16(_) => 0xa461c2db76abc31f,
                    Instr::I32DivS(_) => 0xd8e9ed1b036c4299,
                    Instr::I64DivS(_) => 0xc18a29741fec7821,
                    Instr::I32DivSImm16(_) => 0x85487d90b69b42eb,
                    Instr::I64DivSImm16(_) => 0xdf796fb72044ef89,
                    Instr::I32DivSImm16Rev(_) => 0xb229de81d802ca3d,
                    Instr::I64DivSImm16Rev(_) => 0xbee33b07c1d429e1,
                    Instr::I32DivU(_) => 0x98f910b2a2344797,
                    Instr::I64DivU(_) => 0xe4e5f443dafb6781,
                    Instr::I32DivUImm16(_) => 0xa719ab83a81107c3,
                    Instr::I64DivUImm16(_) => 0xf0a697a43b3d35d7,
                    Instr::I32DivUImm16Rev(_) => 0xdd4813bbc3fe6d13,
                    Instr::I64DivUImm16Rev(_) => 0xb72767906e0a5cfb,
                    Instr::I32RemS(_) => 0x9f03cbda2aa5fa45,
                    Instr::I64RemS(_) => 0xccf3ffab51808eaf,
                    Instr::I32RemSImm16(_) => 0x9d4c371e9aef0583,
                    Instr::I64RemSImm16(_) => 0x947e68335d37e889,
                    Instr::I32RemSImm16Rev(_) => 0x96866c6454a95f1d,
                    Instr::I64RemSImm16Rev(_) => 0x860324e1882094a3,
                    Instr::I32RemU(_) => 0xc543d9e99bfe04db,
                    Instr::I64RemU(_) => 0x9f9e5bd14453abf7,
                    Instr::I32RemUImm16(_) => 0xa0023a1b616065a5,
                    Instr::I64RemUImm16(_) => 0xd45fb600aa0ebecb,
                    Instr::I32RemUImm16Rev(_) => 0xeb6a8bb4c61401c5,
                    Instr::I64RemUImm16Rev(_) => 0xd793fa5aa0a964cd,
                    Instr::I32And(_) => 0xda40caeb3a552221,
                    Instr::I32AndEqz(_) => 0xdae0c4aaf21a2375,
                    Instr::I32AndEqzImm16(_) => 0xe3b2a67a5da4fa6b,
                    Instr::I32AndImm16(_) => 0x8698e382452765f1,
                    Instr::I64And(_) => 0xf96e3bc1640f67cd,
                    Instr::I64AndImm16(_) => 0xcb4730c03868c6c9,
                    Instr::I32Or(_) => 0xfe54c1a4cc88dbfd,
                    Instr::I32OrEqz(_) => 0x81c4ae5533789a77,
                    Instr::I32OrEqzImm16(_) => 0xd5a79e8c5ff0d4f7,
                    Instr::I32OrImm16(_) => 0x907c4d22bec999ad,
                    Instr::I64Or(_) => 0xdc758f1076325dcf,
                    Instr::I64OrImm16(_) => 0xcea9b6298da032eb,
                    Instr::I32Xor(_) => 0x820cea6beee5132b,
                    Instr::I32XorEqz(_) => 0xfb5646a229eba923,
                    Instr::I32XorEqzImm16(_) => 0xa3d2eee0dbef491f,
                    Instr::I32XorImm16(_) => 0xe0802ae028ddf527,
                    Instr::I64Xor(_) => 0xcf8b3ddb8044776f,
                    Instr::I64XorImm16(_) => 0xa84e47947f7723ad,
                    Instr::I32Shl(_) => 0x997deb70c648fa15,
                    Instr::I64Shl(_) => 0x80f706f88d804a25,
                    Instr::I32ShlImm(_) => 0xfcc148faacf59d53,
                    Instr::I64ShlImm(_) => 0xf6d8e1fa8e82bd65,
                    Instr::I32ShlImm16Rev(_) => 0xd9b8925b3c4f7c43,
                    Instr::I64ShlImm16Rev(_) => 0xbb5ab94e3b3b62a1,
                    Instr::I32ShrU(_) => 0x949b3946cd09a095,
                    Instr::I64ShrU(_) => 0xbc8d8ae8fafe0cb5,
                    Instr::I32ShrUImm(_) => 0xdd921c308dd476c5,
                    Instr::I64ShrUImm(_) => 0xb55a52fb3302897d,
                    Instr::I32ShrUImm16Rev(_) => 0xf542ca19c6ede7e5,
                    Instr::I64ShrUImm16Rev(_) => 0xdc4c2c1ee8fd89b9,
                    Instr::I32ShrS(_) => 0xe9925858193a7679,
                    Instr::I64ShrS(_) => 0xfb846cd977392cf3,
                    Instr::I32ShrSImm(_) => 0xc771e42e66e029b7,
                    Instr::I64ShrSImm(_) => 0xd65d3e841e5ef493,
                    Instr::I32ShrSImm16Rev(_) => 0x81f8a0aebcf9ccfb,
                    Instr::I64ShrSImm16Rev(_) => 0xc24ad68846b0f2d7,
                    Instr::I32Rotl(_) => 0xdfd1ecfe45e3e365,
                    Instr::I64Rotl(_) => 0xc2c0280be48f6e2b,
                    Instr::I32RotlImm(_) => 0x821b5a3bc30952e5,
                    Instr::I64RotlImm(_) => 0xb388feacd3e9a985,
                    Instr::I32RotlImm16Rev(_) => 0xe0346c992152e0ad,
                    Instr::I64RotlImm16Rev(_) => 0xbc323dcbfa95b9c1,
                    Instr::I32Rotr(_) => 0xfce450167abfef91,
                    Instr::I64Rotr(_) => 0xd35af32013838db5,
                    Instr::I32RotrImm(_) => 0xb17d776a68c2901d,
                    Instr::I64RotrImm(_) => 0xdc854e900d8c8b91,
                    Instr::I32RotrImm16Rev(_) => 0xc3da2fbf5194e14f,
                    Instr::I64RotrImm16Rev(_) => 0x8ce225f0c3ba867d,
                    Instr::F32Abs(_) => 0xcd5c2fff391d82cb,
                    Instr::F64Abs(_) => 0xc4736057bf6ce827,
                    Instr::F32Neg(_) => 0xd366f959bf938435,
                    Instr::F64Neg(_) => 0x8c01a158032456c5,
                    Instr::F32Ceil(_) => 0xf5684f567a1e5c81,
                    Instr::F64Ceil(_) => 0xbc6729b56b5bf64f,
                    Instr::F32Floor(_) => 0xc3397446971c7b1b,
                    Instr::F64Floor(_) => 0xc21648fabc149443,
                    Instr::F32Trunc(_) => 0x930c87d1457a0b2f,
                    Instr::F64Trunc(_) => 0xc457947a5515448d,
                    Instr::F32Nearest(_) => 0xdcbd8018d58a0133,
                    Instr::F64Nearest(_) => 0xe719d229d8dc9d11,
                    Instr::F32Sqrt(_) => 0x8e031a9674797f6f,
                    Instr::F64Sqrt(_) => 0xede241e1bdbf8add,
                    Instr::F32Add(_) => 0xc0246fd5a4fa2569,
                    Instr::F64Add(_) => 0xae61b186b8d627b1,
                    Instr::F32Sub(_) => 0xf37398a1108c36cb,
                    Instr::F64Sub(_) => 0xaaf86176c0dc89f5,
                    Instr::F32Mul(_) => 0xbe5eb79b83c0c7b1,
                    Instr::F64Mul(_) => 0x9b200d1c1640bf0d,
                    Instr::F32Div(_) => 0xd22d29503c878647,
                    Instr::F64Div(_) => 0x91b08c54e524bb09,
                    Instr::F32Min(_) => 0xf83af276dd4b617f,
                    Instr::F64Min(_) => 0xeb5d7d82375f7be7,
                    Instr::F32Max(_) => 0x8f4d06f60f1c84fb,
                    Instr::F64Max(_) => 0xb24f6877be71ebd5,
                    Instr::F32Copysign(_) => 0xec122620e993dfcd,
                    Instr::F64Copysign(_) => 0xf27f1850006566c9,
                    Instr::F32CopysignImm(_) => 0x94d19450082a4ce9,
                    Instr::F64CopysignImm(_) => 0x84b094c8c0503805,
                    Instr::I32WrapI64(_) => 0xd7348da1051ffdf5,
                    Instr::I64ExtendI32S(_) => 0xbffb8ca25ae4bf8f,
                    Instr::I64ExtendI32U(_) => 0xec15704d37b95ec7,
                    Instr::I32TruncF32S(_) => 0xa8edf1813c31175b,
                    Instr::I32TruncF32U(_) => 0xf980305a8ba3be0f,
                    Instr::I32TruncF64S(_) => 0xb982c8f45dcd5731,
                    Instr::I32TruncF64U(_) => 0x9ca1670d1e934f45,
                    Instr::I64TruncF32S(_) => 0xeb2506c7a7cfe6f7,
                    Instr::I64TruncF32U(_) => 0xa230e0381f36668d,
                    Instr::I64TruncF64S(_) => 0xce02765ab94df325,
                    Instr::I64TruncF64U(_) => 0xb39253799e21a72d,
                    Instr::I32TruncSatF32S(_) => 0xa164fb50eec581d3,
                    Instr::I32TruncSatF32U(_) => 0xabac89637bdb1d8f,
                    Instr::I32TruncSatF64S(_) => 0xe8b8c4421046aedd,
                    Instr::I32TruncSatF64U(_) => 0x91c87015a56a944d,
                    Instr::I64TruncSatF32S(_) => 0xb909e169382afddd,
                    Instr::I64TruncSatF32U(_) => 0xc6f884d2705bf2d3,
                    Instr::I64TruncSatF64S(_) => 0xa5e8386963664fa3,
                    Instr::I64TruncSatF64U(_) => 0xa43800f9e4975aff,
                    Instr::I32Extend8S(_) => 0xdecfc0dc5cb809af,
                    Instr::I32Extend16S(_) => 0xcdf6bb7756026125,
                    Instr::I64Extend8S(_) => 0xb906176cee2380bf,
                    Instr::I64Extend16S(_) => 0xf0382669ed7a55f1,
                    Instr::I64Extend32S(_) => 0xb61b2de5652d06e9,
                    Instr::F32DemoteF64(_) => 0xbf82e5dd4495233b,
                    Instr::F64PromoteF32(_) => 0xf42a79d7ed7c17c3,
                    Instr::F32ConvertI32S(_) => 0x9e65030287165e29,
                    Instr::F32ConvertI32U(_) => 0xe244f0acd2209f0b,
                    Instr::F32ConvertI64S(_) => 0xd007d6d9333c7405,
                    Instr::F32ConvertI64U(_) => 0xf31a7af87a7b7f61,
                    Instr::F64ConvertI32S(_) => 0xbef1a0dfa7540b4d,
                    Instr::F64ConvertI32U(_) => 0xa168200e59e18dcd,
                    Instr::F64ConvertI64S(_) => 0xb3a2d5946ee565e3,
                    Instr::F64ConvertI64U(_) => 0x92ad2f2873e8fbc5,
                };
                self.update_runtime_signature(instr_prime);
            }
            match instr {
                Instr::TableIdx(_)
                | Instr::DataSegmentIdx(_)
                | Instr::ElementSegmentIdx(_)
                | Instr::Const32(_)
                | Instr::I64Const32(_)
                | Instr::F64Const32(_)
                | Instr::Register(_)
                | Instr::Register2(_)
                | Instr::Register3(_)
                | Instr::RegisterList(_)
                | Instr::CallIndirectParams(_)
                | Instr::CallIndirectParamsImm16(_) => self.invalid_instruction_word()?,
                Instr::Trap(trap_code) => self.execute_trap(trap_code)?,
                Instr::ConsumeFuel(block_fuel) => self.execute_consume_fuel(block_fuel)?,
                Instr::Return => {
                    forward_return!(self.execute_return())
                }
                Instr::ReturnReg { value } => {
                    forward_return!(self.execute_return_reg(value))
                }
                Instr::ReturnReg2 { values } => {
                    forward_return!(self.execute_return_reg2(values))
                }
                Instr::ReturnReg3 { values } => {
                    forward_return!(self.execute_return_reg3(values))
                }
                Instr::ReturnImm32 { value } => {
                    forward_return!(self.execute_return_imm32(value))
                }
                Instr::ReturnI64Imm32 { value } => {
                    forward_return!(self.execute_return_i64imm32(value))
                }
                Instr::ReturnF64Imm32 { value } => {
                    forward_return!(self.execute_return_f64imm32(value))
                }
                Instr::ReturnSpan { values } => {
                    forward_return!(self.execute_return_span(values))
                }
                Instr::ReturnMany { values } => {
                    forward_return!(self.execute_return_many(values))
                }
                Instr::ReturnNez { condition } => {
                    forward_return!(self.execute_return_nez(condition))
                }
                Instr::ReturnNezReg { condition, value } => {
                    forward_return!(self.execute_return_nez_reg(condition, value))
                }
                Instr::ReturnNezReg2 { condition, values } => {
                    forward_return!(self.execute_return_nez_reg2(condition, values))
                }
                Instr::ReturnNezImm32 { condition, value } => {
                    forward_return!(self.execute_return_nez_imm32(condition, value))
                }
                Instr::ReturnNezI64Imm32 { condition, value } => {
                    forward_return!(self.execute_return_nez_i64imm32(condition, value))
                }
                Instr::ReturnNezF64Imm32 { condition, value } => {
                    forward_return!(self.execute_return_nez_f64imm32(condition, value))
                }
                Instr::ReturnNezSpan { condition, values } => {
                    forward_return!(self.execute_return_nez_span(condition, values))
                }
                Instr::ReturnNezMany { condition, values } => {
                    forward_return!(self.execute_return_nez_many(condition, values))
                }
                Instr::Branch { offset } => self.execute_branch(offset),
                Instr::BranchTable { index, len_targets } => {
                    self.execute_branch_table(index, len_targets)
                }
                Instr::BranchCmpFallback { lhs, rhs, params } => {
                    self.execute_branch_cmp_fallback(lhs, rhs, params)
                }
                Instr::BranchI32And(instr) => self.execute_branch_i32_and(instr),
                Instr::BranchI32AndImm(instr) => self.execute_branch_i32_and_imm(instr),
                Instr::BranchI32Or(instr) => self.execute_branch_i32_or(instr),
                Instr::BranchI32OrImm(instr) => self.execute_branch_i32_or_imm(instr),
                Instr::BranchI32Xor(instr) => self.execute_branch_i32_xor(instr),
                Instr::BranchI32XorImm(instr) => self.execute_branch_i32_xor_imm(instr),
                Instr::BranchI32AndEqz(instr) => self.execute_branch_i32_and_eqz(instr),
                Instr::BranchI32AndEqzImm(instr) => self.execute_branch_i32_and_eqz_imm(instr),
                Instr::BranchI32OrEqz(instr) => self.execute_branch_i32_or_eqz(instr),
                Instr::BranchI32OrEqzImm(instr) => self.execute_branch_i32_or_eqz_imm(instr),
                Instr::BranchI32XorEqz(instr) => self.execute_branch_i32_xor_eqz(instr),
                Instr::BranchI32XorEqzImm(instr) => self.execute_branch_i32_xor_eqz_imm(instr),
                Instr::BranchI32Eq(instr) => self.execute_branch_i32_eq(instr),
                Instr::BranchI32EqImm(instr) => self.execute_branch_i32_eq_imm(instr),
                Instr::BranchI32Ne(instr) => self.execute_branch_i32_ne(instr),
                Instr::BranchI32NeImm(instr) => self.execute_branch_i32_ne_imm(instr),
                Instr::BranchI32LtS(instr) => self.execute_branch_i32_lt_s(instr),
                Instr::BranchI32LtSImm(instr) => self.execute_branch_i32_lt_s_imm(instr),
                Instr::BranchI32LtU(instr) => self.execute_branch_i32_lt_u(instr),
                Instr::BranchI32LtUImm(instr) => self.execute_branch_i32_lt_u_imm(instr),
                Instr::BranchI32LeS(instr) => self.execute_branch_i32_le_s(instr),
                Instr::BranchI32LeSImm(instr) => self.execute_branch_i32_le_s_imm(instr),
                Instr::BranchI32LeU(instr) => self.execute_branch_i32_le_u(instr),
                Instr::BranchI32LeUImm(instr) => self.execute_branch_i32_le_u_imm(instr),
                Instr::BranchI32GtS(instr) => self.execute_branch_i32_gt_s(instr),
                Instr::BranchI32GtSImm(instr) => self.execute_branch_i32_gt_s_imm(instr),
                Instr::BranchI32GtU(instr) => self.execute_branch_i32_gt_u(instr),
                Instr::BranchI32GtUImm(instr) => self.execute_branch_i32_gt_u_imm(instr),
                Instr::BranchI32GeS(instr) => self.execute_branch_i32_ge_s(instr),
                Instr::BranchI32GeSImm(instr) => self.execute_branch_i32_ge_s_imm(instr),
                Instr::BranchI32GeU(instr) => self.execute_branch_i32_ge_u(instr),
                Instr::BranchI32GeUImm(instr) => self.execute_branch_i32_ge_u_imm(instr),
                Instr::BranchI64Eq(instr) => self.execute_branch_i64_eq(instr),
                Instr::BranchI64EqImm(instr) => self.execute_branch_i64_eq_imm(instr),
                Instr::BranchI64Ne(instr) => self.execute_branch_i64_ne(instr),
                Instr::BranchI64NeImm(instr) => self.execute_branch_i64_ne_imm(instr),
                Instr::BranchI64LtS(instr) => self.execute_branch_i64_lt_s(instr),
                Instr::BranchI64LtSImm(instr) => self.execute_branch_i64_lt_s_imm(instr),
                Instr::BranchI64LtU(instr) => self.execute_branch_i64_lt_u(instr),
                Instr::BranchI64LtUImm(instr) => self.execute_branch_i64_lt_u_imm(instr),
                Instr::BranchI64LeS(instr) => self.execute_branch_i64_le_s(instr),
                Instr::BranchI64LeSImm(instr) => self.execute_branch_i64_le_s_imm(instr),
                Instr::BranchI64LeU(instr) => self.execute_branch_i64_le_u(instr),
                Instr::BranchI64LeUImm(instr) => self.execute_branch_i64_le_u_imm(instr),
                Instr::BranchI64GtS(instr) => self.execute_branch_i64_gt_s(instr),
                Instr::BranchI64GtSImm(instr) => self.execute_branch_i64_gt_s_imm(instr),
                Instr::BranchI64GtU(instr) => self.execute_branch_i64_gt_u(instr),
                Instr::BranchI64GtUImm(instr) => self.execute_branch_i64_gt_u_imm(instr),
                Instr::BranchI64GeS(instr) => self.execute_branch_i64_ge_s(instr),
                Instr::BranchI64GeSImm(instr) => self.execute_branch_i64_ge_s_imm(instr),
                Instr::BranchI64GeU(instr) => self.execute_branch_i64_ge_u(instr),
                Instr::BranchI64GeUImm(instr) => self.execute_branch_i64_ge_u_imm(instr),
                Instr::BranchF32Eq(instr) => self.execute_branch_f32_eq(instr),
                Instr::BranchF32Ne(instr) => self.execute_branch_f32_ne(instr),
                Instr::BranchF32Lt(instr) => self.execute_branch_f32_lt(instr),
                Instr::BranchF32Le(instr) => self.execute_branch_f32_le(instr),
                Instr::BranchF32Gt(instr) => self.execute_branch_f32_gt(instr),
                Instr::BranchF32Ge(instr) => self.execute_branch_f32_ge(instr),
                Instr::BranchF64Eq(instr) => self.execute_branch_f64_eq(instr),
                Instr::BranchF64Ne(instr) => self.execute_branch_f64_ne(instr),
                Instr::BranchF64Lt(instr) => self.execute_branch_f64_lt(instr),
                Instr::BranchF64Le(instr) => self.execute_branch_f64_le(instr),
                Instr::BranchF64Gt(instr) => self.execute_branch_f64_gt(instr),
                Instr::BranchF64Ge(instr) => self.execute_branch_f64_ge(instr),
                Instr::Copy { result, value } => self.execute_copy(result, value),
                Instr::Copy2 { results, values } => self.execute_copy_2(results, values),
                Instr::CopyImm32 { result, value } => self.execute_copy_imm32(result, value),
                Instr::CopyI64Imm32 { result, value } => self.execute_copy_i64imm32(result, value),
                Instr::CopyF64Imm32 { result, value } => self.execute_copy_f64imm32(result, value),
                Instr::CopySpan {
                    results,
                    values,
                    len,
                } => self.execute_copy_span(results, values, len),
                Instr::CopySpanNonOverlapping {
                    results,
                    values,
                    len,
                } => self.execute_copy_span_non_overlapping(results, values, len),
                Instr::CopyMany { results, values } => self.execute_copy_many(results, values),
                Instr::CopyManyNonOverlapping { results, values } => {
                    self.execute_copy_many_non_overlapping(results, values)
                }
                Instr::ReturnCallInternal0 { func } => self.execute_return_call_internal_0(func)?,
                Instr::ReturnCallInternal { func } => self.execute_return_call_internal(func)?,
                Instr::ReturnCallImported0 { func } => {
                    forward_call!(self.execute_return_call_imported_0(func))
                }
                Instr::ReturnCallImported { func } => {
                    forward_call!(self.execute_return_call_imported(func))
                }
                Instr::ReturnCallIndirect0 { func_type } => {
                    forward_call!(self.execute_return_call_indirect_0(func_type))
                }
                Instr::ReturnCallIndirect { func_type } => {
                    forward_call!(self.execute_return_call_indirect(func_type))
                }
                Instr::CallInternal0 { results, func } => {
                    self.execute_call_internal_0(results, func)?
                }
                Instr::CallInternal { results, func } => {
                    self.execute_call_internal(results, func)?
                }
                Instr::CallImported0 { results, func } => {
                    forward_call!(self.execute_call_imported_0(results, func))
                }
                Instr::CallImported { results, func } => {
                    forward_call!(self.execute_call_imported(results, func))
                }
                Instr::CallIndirect0 { results, func_type } => {
                    forward_call!(self.execute_call_indirect_0(results, func_type))
                }
                Instr::CallIndirect { results, func_type } => {
                    forward_call!(self.execute_call_indirect(results, func_type))
                }
                Instr::Select {
                    result,
                    condition,
                    lhs,
                } => self.execute_select(result, condition, lhs),
                Instr::SelectRev {
                    result,
                    condition,
                    rhs,
                } => self.execute_select_rev(result, condition, rhs),
                Instr::SelectImm32 {
                    result_or_condition,
                    lhs_or_rhs,
                } => self.execute_select_imm32(result_or_condition, lhs_or_rhs),
                Instr::SelectI64Imm32 {
                    result_or_condition,
                    lhs_or_rhs,
                } => self.execute_select_i64imm32(result_or_condition, lhs_or_rhs),
                Instr::SelectF64Imm32 {
                    result_or_condition,
                    lhs_or_rhs,
                } => self.execute_select_f64imm32(result_or_condition, lhs_or_rhs),
                Instr::RefFunc { result, func } => self.execute_ref_func(result, func),
                Instr::TableGet { result, index } => self.execute_table_get(result, index)?,
                Instr::TableGetImm { result, index } => {
                    self.execute_table_get_imm(result, index)?
                }
                Instr::TableSize { result, table } => self.execute_table_size(result, table),
                Instr::TableSet { index, value } => self.execute_table_set(index, value)?,
                Instr::TableSetAt { index, value } => self.execute_table_set_at(index, value)?,
                Instr::TableCopy { dst, src, len } => self.execute_table_copy(dst, src, len)?,
                Instr::TableCopyTo { dst, src, len } => {
                    self.execute_table_copy_to(dst, src, len)?
                }
                Instr::TableCopyFrom { dst, src, len } => {
                    self.execute_table_copy_from(dst, src, len)?
                }
                Instr::TableCopyFromTo { dst, src, len } => {
                    self.execute_table_copy_from_to(dst, src, len)?
                }
                Instr::TableCopyExact { dst, src, len } => {
                    self.execute_table_copy_exact(dst, src, len)?
                }
                Instr::TableCopyToExact { dst, src, len } => {
                    self.execute_table_copy_to_exact(dst, src, len)?
                }
                Instr::TableCopyFromExact { dst, src, len } => {
                    self.execute_table_copy_from_exact(dst, src, len)?
                }
                Instr::TableCopyFromToExact { dst, src, len } => {
                    self.execute_table_copy_from_to_exact(dst, src, len)?
                }
                Instr::TableInit { dst, src, len } => self.execute_table_init(dst, src, len)?,
                Instr::TableInitTo { dst, src, len } => {
                    self.execute_table_init_to(dst, src, len)?
                }
                Instr::TableInitFrom { dst, src, len } => {
                    self.execute_table_init_from(dst, src, len)?
                }
                Instr::TableInitFromTo { dst, src, len } => {
                    self.execute_table_init_from_to(dst, src, len)?
                }
                Instr::TableInitExact { dst, src, len } => {
                    self.execute_table_init_exact(dst, src, len)?
                }
                Instr::TableInitToExact { dst, src, len } => {
                    self.execute_table_init_to_exact(dst, src, len)?
                }
                Instr::TableInitFromExact { dst, src, len } => {
                    self.execute_table_init_from_exact(dst, src, len)?
                }
                Instr::TableInitFromToExact { dst, src, len } => {
                    self.execute_table_init_from_to_exact(dst, src, len)?
                }
                Instr::TableFill { dst, len, value } => self.execute_table_fill(dst, len, value)?,
                Instr::TableFillAt { dst, len, value } => {
                    self.execute_table_fill_at(dst, len, value)?
                }
                Instr::TableFillExact { dst, len, value } => {
                    self.execute_table_fill_exact(dst, len, value)?
                }
                Instr::TableFillAtExact { dst, len, value } => {
                    self.execute_table_fill_at_exact(dst, len, value)?
                }
                Instr::TableGrow {
                    result,
                    delta,
                    value,
                } => self.execute_table_grow(result, delta, value, &mut *resource_limiter)?,
                Instr::TableGrowImm {
                    result,
                    delta,
                    value,
                } => self.execute_table_grow_imm(result, delta, value, &mut *resource_limiter)?,
                Instr::ElemDrop(element_index) => self.execute_element_drop(element_index),
                Instr::DataDrop(data_index) => self.execute_data_drop(data_index),
                Instr::MemorySize { result } => self.execute_memory_size(result),
                Instr::MemoryGrow { result, delta } => {
                    self.execute_memory_grow(result, delta, &mut *resource_limiter)?
                }
                Instr::MemoryGrowBy { result, delta } => {
                    self.execute_memory_grow_by(result, delta, &mut *resource_limiter)?
                }
                Instr::MemoryCopy { dst, src, len } => self.execute_memory_copy(dst, src, len)?,
                Instr::MemoryCopyTo { dst, src, len } => {
                    self.execute_memory_copy_to(dst, src, len)?
                }
                Instr::MemoryCopyFrom { dst, src, len } => {
                    self.execute_memory_copy_from(dst, src, len)?
                }
                Instr::MemoryCopyFromTo { dst, src, len } => {
                    self.execute_memory_copy_from_to(dst, src, len)?
                }
                Instr::MemoryCopyExact { dst, src, len } => {
                    self.execute_memory_copy_exact(dst, src, len)?
                }
                Instr::MemoryCopyToExact { dst, src, len } => {
                    self.execute_memory_copy_to_exact(dst, src, len)?
                }
                Instr::MemoryCopyFromExact { dst, src, len } => {
                    self.execute_memory_copy_from_exact(dst, src, len)?
                }
                Instr::MemoryCopyFromToExact { dst, src, len } => {
                    self.execute_memory_copy_from_to_exact(dst, src, len)?
                }
                Instr::MemoryFill { dst, value, len } => {
                    self.execute_memory_fill(dst, value, len)?
                }
                Instr::MemoryFillAt { dst, value, len } => {
                    self.execute_memory_fill_at(dst, value, len)?
                }
                Instr::MemoryFillImm { dst, value, len } => {
                    self.execute_memory_fill_imm(dst, value, len)?
                }
                Instr::MemoryFillExact { dst, value, len } => {
                    self.execute_memory_fill_exact(dst, value, len)?
                }
                Instr::MemoryFillAtImm { dst, value, len } => {
                    self.execute_memory_fill_at_imm(dst, value, len)?
                }
                Instr::MemoryFillAtExact { dst, value, len } => {
                    self.execute_memory_fill_at_exact(dst, value, len)?
                }
                Instr::MemoryFillImmExact { dst, value, len } => {
                    self.execute_memory_fill_imm_exact(dst, value, len)?
                }
                Instr::MemoryFillAtImmExact { dst, value, len } => {
                    self.execute_memory_fill_at_imm_exact(dst, value, len)?
                }
                Instr::MemoryInit { dst, src, len } => self.execute_memory_init(dst, src, len)?,
                Instr::MemoryInitTo { dst, src, len } => {
                    self.execute_memory_init_to(dst, src, len)?
                }
                Instr::MemoryInitFrom { dst, src, len } => {
                    self.execute_memory_init_from(dst, src, len)?
                }
                Instr::MemoryInitFromTo { dst, src, len } => {
                    self.execute_memory_init_from_to(dst, src, len)?
                }
                Instr::MemoryInitExact { dst, src, len } => {
                    self.execute_memory_init_exact(dst, src, len)?
                }
                Instr::MemoryInitToExact { dst, src, len } => {
                    self.execute_memory_init_to_exact(dst, src, len)?
                }
                Instr::MemoryInitFromExact { dst, src, len } => {
                    self.execute_memory_init_from_exact(dst, src, len)?
                }
                Instr::MemoryInitFromToExact { dst, src, len } => {
                    self.execute_memory_init_from_to_exact(dst, src, len)?
                }
                Instr::GlobalGet { result, global } => self.execute_global_get(result, global),
                Instr::GlobalSet { global, input } => self.execute_global_set(global, input),
                Instr::GlobalSetI32Imm16 { global, input } => {
                    self.execute_global_set_i32imm16(global, input)
                }
                Instr::GlobalSetI64Imm16 { global, input } => {
                    self.execute_global_set_i64imm16(global, input)
                }
                Instr::I32Load(instr) => self.execute_i32_load(instr)?,
                Instr::I32LoadAt(instr) => self.execute_i32_load_at(instr)?,
                Instr::I32LoadOffset16(instr) => self.execute_i32_load_offset16(instr)?,
                Instr::I64Load(instr) => self.execute_i64_load(instr)?,
                Instr::I64LoadAt(instr) => self.execute_i64_load_at(instr)?,
                Instr::I64LoadOffset16(instr) => self.execute_i64_load_offset16(instr)?,
                Instr::F32Load(instr) => self.execute_f32_load(instr)?,
                Instr::F32LoadAt(instr) => self.execute_f32_load_at(instr)?,
                Instr::F32LoadOffset16(instr) => self.execute_f32_load_offset16(instr)?,
                Instr::F64Load(instr) => self.execute_f64_load(instr)?,
                Instr::F64LoadAt(instr) => self.execute_f64_load_at(instr)?,
                Instr::F64LoadOffset16(instr) => self.execute_f64_load_offset16(instr)?,
                Instr::I32Load8s(instr) => self.execute_i32_load8_s(instr)?,
                Instr::I32Load8sAt(instr) => self.execute_i32_load8_s_at(instr)?,
                Instr::I32Load8sOffset16(instr) => self.execute_i32_load8_s_offset16(instr)?,
                Instr::I32Load8u(instr) => self.execute_i32_load8_u(instr)?,
                Instr::I32Load8uAt(instr) => self.execute_i32_load8_u_at(instr)?,
                Instr::I32Load8uOffset16(instr) => self.execute_i32_load8_u_offset16(instr)?,
                Instr::I32Load16s(instr) => self.execute_i32_load16_s(instr)?,
                Instr::I32Load16sAt(instr) => self.execute_i32_load16_s_at(instr)?,
                Instr::I32Load16sOffset16(instr) => self.execute_i32_load16_s_offset16(instr)?,
                Instr::I32Load16u(instr) => self.execute_i32_load16_u(instr)?,
                Instr::I32Load16uAt(instr) => self.execute_i32_load16_u_at(instr)?,
                Instr::I32Load16uOffset16(instr) => self.execute_i32_load16_u_offset16(instr)?,
                Instr::I64Load8s(instr) => self.execute_i64_load8_s(instr)?,
                Instr::I64Load8sAt(instr) => self.execute_i64_load8_s_at(instr)?,
                Instr::I64Load8sOffset16(instr) => self.execute_i64_load8_s_offset16(instr)?,
                Instr::I64Load8u(instr) => self.execute_i64_load8_u(instr)?,
                Instr::I64Load8uAt(instr) => self.execute_i64_load8_u_at(instr)?,
                Instr::I64Load8uOffset16(instr) => self.execute_i64_load8_u_offset16(instr)?,
                Instr::I64Load16s(instr) => self.execute_i64_load16_s(instr)?,
                Instr::I64Load16sAt(instr) => self.execute_i64_load16_s_at(instr)?,
                Instr::I64Load16sOffset16(instr) => self.execute_i64_load16_s_offset16(instr)?,
                Instr::I64Load16u(instr) => self.execute_i64_load16_u(instr)?,
                Instr::I64Load16uAt(instr) => self.execute_i64_load16_u_at(instr)?,
                Instr::I64Load16uOffset16(instr) => self.execute_i64_load16_u_offset16(instr)?,
                Instr::I64Load32s(instr) => self.execute_i64_load32_s(instr)?,
                Instr::I64Load32sAt(instr) => self.execute_i64_load32_s_at(instr)?,
                Instr::I64Load32sOffset16(instr) => self.execute_i64_load32_s_offset16(instr)?,
                Instr::I64Load32u(instr) => self.execute_i64_load32_u(instr)?,
                Instr::I64Load32uAt(instr) => self.execute_i64_load32_u_at(instr)?,
                Instr::I64Load32uOffset16(instr) => self.execute_i64_load32_u_offset16(instr)?,
                Instr::I32Store(instr) => self.execute_i32_store(instr)?,
                Instr::I32StoreOffset16(instr) => self.execute_i32_store_offset16(instr)?,
                Instr::I32StoreOffset16Imm16(instr) => {
                    self.execute_i32_store_offset16_imm16(instr)?
                }
                Instr::I32StoreAt(instr) => self.execute_i32_store_at(instr)?,
                Instr::I32StoreAtImm16(instr) => self.execute_i32_store_at_imm16(instr)?,
                Instr::I32Store8(instr) => self.execute_i32_store8(instr)?,
                Instr::I32Store8Offset16(instr) => self.execute_i32_store8_offset16(instr)?,
                Instr::I32Store8Offset16Imm(instr) => {
                    self.execute_i32_store8_offset16_imm(instr)?
                }
                Instr::I32Store8At(instr) => self.execute_i32_store8_at(instr)?,
                Instr::I32Store8AtImm(instr) => self.execute_i32_store8_at_imm(instr)?,
                Instr::I32Store16(instr) => self.execute_i32_store16(instr)?,
                Instr::I32Store16Offset16(instr) => self.execute_i32_store16_offset16(instr)?,
                Instr::I32Store16Offset16Imm(instr) => {
                    self.execute_i32_store16_offset16_imm(instr)?
                }
                Instr::I32Store16At(instr) => self.execute_i32_store16_at(instr)?,
                Instr::I32Store16AtImm(instr) => self.execute_i32_store16_at_imm(instr)?,
                Instr::I64Store(instr) => self.execute_i64_store(instr)?,
                Instr::I64StoreOffset16(instr) => self.execute_i64_store_offset16(instr)?,
                Instr::I64StoreOffset16Imm16(instr) => {
                    self.execute_i64_store_offset16_imm16(instr)?
                }
                Instr::I64StoreAt(instr) => self.execute_i64_store_at(instr)?,
                Instr::I64StoreAtImm16(instr) => self.execute_i64_store_at_imm16(instr)?,
                Instr::I64Store8(instr) => self.execute_i64_store8(instr)?,
                Instr::I64Store8Offset16(instr) => self.execute_i64_store8_offset16(instr)?,
                Instr::I64Store8Offset16Imm(instr) => {
                    self.execute_i64_store8_offset16_imm(instr)?
                }
                Instr::I64Store8At(instr) => self.execute_i64_store8_at(instr)?,
                Instr::I64Store8AtImm(instr) => self.execute_i64_store8_at_imm(instr)?,
                Instr::I64Store16(instr) => self.execute_i64_store16(instr)?,
                Instr::I64Store16Offset16(instr) => self.execute_i64_store16_offset16(instr)?,
                Instr::I64Store16Offset16Imm(instr) => {
                    self.execute_i64_store16_offset16_imm(instr)?
                }
                Instr::I64Store16At(instr) => self.execute_i64_store16_at(instr)?,
                Instr::I64Store16AtImm(instr) => self.execute_i64_store16_at_imm(instr)?,
                Instr::I64Store32(instr) => self.execute_i64_store32(instr)?,
                Instr::I64Store32Offset16(instr) => self.execute_i64_store32_offset16(instr)?,
                Instr::I64Store32Offset16Imm16(instr) => {
                    self.execute_i64_store32_offset16_imm16(instr)?
                }
                Instr::I64Store32At(instr) => self.execute_i64_store32_at(instr)?,
                Instr::I64Store32AtImm16(instr) => self.execute_i64_store32_at_imm16(instr)?,
                Instr::F32Store(instr) => self.execute_f32_store(instr)?,
                Instr::F32StoreOffset16(instr) => self.execute_f32_store_offset16(instr)?,
                Instr::F32StoreAt(instr) => self.execute_f32_store_at(instr)?,
                Instr::F64Store(instr) => self.execute_f64_store(instr)?,
                Instr::F64StoreOffset16(instr) => self.execute_f64_store_offset16(instr)?,
                Instr::F64StoreAt(instr) => self.execute_f64_store_at(instr)?,
                Instr::I32Eq(instr) => self.execute_i32_eq(instr),
                Instr::I32EqImm16(instr) => self.execute_i32_eq_imm16(instr),
                Instr::I32Ne(instr) => self.execute_i32_ne(instr),
                Instr::I32NeImm16(instr) => self.execute_i32_ne_imm16(instr),
                Instr::I32LtS(instr) => self.execute_i32_lt_s(instr),
                Instr::I32LtSImm16(instr) => self.execute_i32_lt_s_imm16(instr),
                Instr::I32LtU(instr) => self.execute_i32_lt_u(instr),
                Instr::I32LtUImm16(instr) => self.execute_i32_lt_u_imm16(instr),
                Instr::I32LeS(instr) => self.execute_i32_le_s(instr),
                Instr::I32LeSImm16(instr) => self.execute_i32_le_s_imm16(instr),
                Instr::I32LeU(instr) => self.execute_i32_le_u(instr),
                Instr::I32LeUImm16(instr) => self.execute_i32_le_u_imm16(instr),
                Instr::I32GtS(instr) => self.execute_i32_gt_s(instr),
                Instr::I32GtSImm16(instr) => self.execute_i32_gt_s_imm16(instr),
                Instr::I32GtU(instr) => self.execute_i32_gt_u(instr),
                Instr::I32GtUImm16(instr) => self.execute_i32_gt_u_imm16(instr),
                Instr::I32GeS(instr) => self.execute_i32_ge_s(instr),
                Instr::I32GeSImm16(instr) => self.execute_i32_ge_s_imm16(instr),
                Instr::I32GeU(instr) => self.execute_i32_ge_u(instr),
                Instr::I32GeUImm16(instr) => self.execute_i32_ge_u_imm16(instr),
                Instr::I64Eq(instr) => self.execute_i64_eq(instr),
                Instr::I64EqImm16(instr) => self.execute_i64_eq_imm16(instr),
                Instr::I64Ne(instr) => self.execute_i64_ne(instr),
                Instr::I64NeImm16(instr) => self.execute_i64_ne_imm16(instr),
                Instr::I64LtS(instr) => self.execute_i64_lt_s(instr),
                Instr::I64LtSImm16(instr) => self.execute_i64_lt_s_imm16(instr),
                Instr::I64LtU(instr) => self.execute_i64_lt_u(instr),
                Instr::I64LtUImm16(instr) => self.execute_i64_lt_u_imm16(instr),
                Instr::I64LeS(instr) => self.execute_i64_le_s(instr),
                Instr::I64LeSImm16(instr) => self.execute_i64_le_s_imm16(instr),
                Instr::I64LeU(instr) => self.execute_i64_le_u(instr),
                Instr::I64LeUImm16(instr) => self.execute_i64_le_u_imm16(instr),
                Instr::I64GtS(instr) => self.execute_i64_gt_s(instr),
                Instr::I64GtSImm16(instr) => self.execute_i64_gt_s_imm16(instr),
                Instr::I64GtU(instr) => self.execute_i64_gt_u(instr),
                Instr::I64GtUImm16(instr) => self.execute_i64_gt_u_imm16(instr),
                Instr::I64GeS(instr) => self.execute_i64_ge_s(instr),
                Instr::I64GeSImm16(instr) => self.execute_i64_ge_s_imm16(instr),
                Instr::I64GeU(instr) => self.execute_i64_ge_u(instr),
                Instr::I64GeUImm16(instr) => self.execute_i64_ge_u_imm16(instr),
                Instr::F32Eq(instr) => self.execute_f32_eq(instr),
                Instr::F32Ne(instr) => self.execute_f32_ne(instr),
                Instr::F32Lt(instr) => self.execute_f32_lt(instr),
                Instr::F32Le(instr) => self.execute_f32_le(instr),
                Instr::F32Gt(instr) => self.execute_f32_gt(instr),
                Instr::F32Ge(instr) => self.execute_f32_ge(instr),
                Instr::F64Eq(instr) => self.execute_f64_eq(instr),
                Instr::F64Ne(instr) => self.execute_f64_ne(instr),
                Instr::F64Lt(instr) => self.execute_f64_lt(instr),
                Instr::F64Le(instr) => self.execute_f64_le(instr),
                Instr::F64Gt(instr) => self.execute_f64_gt(instr),
                Instr::F64Ge(instr) => self.execute_f64_ge(instr),
                Instr::I32Clz(instr) => self.execute_i32_clz(instr),
                Instr::I64Clz(instr) => self.execute_i64_clz(instr),
                Instr::I32Ctz(instr) => self.execute_i32_ctz(instr),
                Instr::I64Ctz(instr) => self.execute_i64_ctz(instr),
                Instr::I32Popcnt(instr) => self.execute_i32_popcnt(instr),
                Instr::I64Popcnt(instr) => self.execute_i64_popcnt(instr),
                Instr::I32Add(instr) => self.execute_i32_add(instr),
                Instr::I32AddImm16(instr) => self.execute_i32_add_imm16(instr),
                Instr::I32Sub(instr) => self.execute_i32_sub(instr),
                Instr::I32SubImm16(instr) => self.execute_i32_sub_imm16(instr),
                Instr::I32SubImm16Rev(instr) => self.execute_i32_sub_imm16_rev(instr),
                Instr::I32Mul(instr) => self.execute_i32_mul(instr),
                Instr::I32MulImm16(instr) => self.execute_i32_mul_imm16(instr),
                Instr::I32DivS(instr) => self.execute_i32_div_s(instr)?,
                Instr::I32DivSImm16(instr) => self.execute_i32_div_s_imm16(instr)?,
                Instr::I32DivSImm16Rev(instr) => self.execute_i32_div_s_imm16_rev(instr)?,
                Instr::I32DivU(instr) => self.execute_i32_div_u(instr)?,
                Instr::I32DivUImm16(instr) => self.execute_i32_div_u_imm16(instr),
                Instr::I32DivUImm16Rev(instr) => self.execute_i32_div_u_imm16_rev(instr)?,
                Instr::I32RemS(instr) => self.execute_i32_rem_s(instr)?,
                Instr::I32RemSImm16(instr) => self.execute_i32_rem_s_imm16(instr)?,
                Instr::I32RemSImm16Rev(instr) => self.execute_i32_rem_s_imm16_rev(instr)?,
                Instr::I32RemU(instr) => self.execute_i32_rem_u(instr)?,
                Instr::I32RemUImm16(instr) => self.execute_i32_rem_u_imm16(instr),
                Instr::I32RemUImm16Rev(instr) => self.execute_i32_rem_u_imm16_rev(instr)?,
                Instr::I32And(instr) => self.execute_i32_and(instr),
                Instr::I32AndEqz(instr) => self.execute_i32_and_eqz(instr),
                Instr::I32AndEqzImm16(instr) => self.execute_i32_and_eqz_imm16(instr),
                Instr::I32AndImm16(instr) => self.execute_i32_and_imm16(instr),
                Instr::I32Or(instr) => self.execute_i32_or(instr),
                Instr::I32OrEqz(instr) => self.execute_i32_or_eqz(instr),
                Instr::I32OrEqzImm16(instr) => self.execute_i32_or_eqz_imm16(instr),
                Instr::I32OrImm16(instr) => self.execute_i32_or_imm16(instr),
                Instr::I32Xor(instr) => self.execute_i32_xor(instr),
                Instr::I32XorEqz(instr) => self.execute_i32_xor_eqz(instr),
                Instr::I32XorEqzImm16(instr) => self.execute_i32_xor_eqz_imm16(instr),
                Instr::I32XorImm16(instr) => self.execute_i32_xor_imm16(instr),
                Instr::I32Shl(instr) => self.execute_i32_shl(instr),
                Instr::I32ShlImm(instr) => self.execute_i32_shl_imm(instr),
                Instr::I32ShlImm16Rev(instr) => self.execute_i32_shl_imm16_rev(instr),
                Instr::I32ShrU(instr) => self.execute_i32_shr_u(instr),
                Instr::I32ShrUImm(instr) => self.execute_i32_shr_u_imm(instr),
                Instr::I32ShrUImm16Rev(instr) => self.execute_i32_shr_u_imm16_rev(instr),
                Instr::I32ShrS(instr) => self.execute_i32_shr_s(instr),
                Instr::I32ShrSImm(instr) => self.execute_i32_shr_s_imm(instr),
                Instr::I32ShrSImm16Rev(instr) => self.execute_i32_shr_s_imm16_rev(instr),
                Instr::I32Rotl(instr) => self.execute_i32_rotl(instr),
                Instr::I32RotlImm(instr) => self.execute_i32_rotl_imm(instr),
                Instr::I32RotlImm16Rev(instr) => self.execute_i32_rotl_imm16_rev(instr),
                Instr::I32Rotr(instr) => self.execute_i32_rotr(instr),
                Instr::I32RotrImm(instr) => self.execute_i32_rotr_imm(instr),
                Instr::I32RotrImm16Rev(instr) => self.execute_i32_rotr_imm16_rev(instr),
                Instr::I64Add(instr) => self.execute_i64_add(instr),
                Instr::I64AddImm16(instr) => self.execute_i64_add_imm16(instr),
                Instr::I64Sub(instr) => self.execute_i64_sub(instr),
                Instr::I64SubImm16(instr) => self.execute_i64_sub_imm16(instr),
                Instr::I64SubImm16Rev(instr) => self.execute_i64_sub_imm16_rev(instr),
                Instr::I64Mul(instr) => self.execute_i64_mul(instr),
                Instr::I64MulImm16(instr) => self.execute_i64_mul_imm16(instr),
                Instr::I64DivS(instr) => self.execute_i64_div_s(instr)?,
                Instr::I64DivSImm16(instr) => self.execute_i64_div_s_imm16(instr)?,
                Instr::I64DivSImm16Rev(instr) => self.execute_i64_div_s_imm16_rev(instr)?,
                Instr::I64DivU(instr) => self.execute_i64_div_u(instr)?,
                Instr::I64DivUImm16(instr) => self.execute_i64_div_u_imm16(instr),
                Instr::I64DivUImm16Rev(instr) => self.execute_i64_div_u_imm16_rev(instr)?,
                Instr::I64RemS(instr) => self.execute_i64_rem_s(instr)?,
                Instr::I64RemSImm16(instr) => self.execute_i64_rem_s_imm16(instr)?,
                Instr::I64RemSImm16Rev(instr) => self.execute_i64_rem_s_imm16_rev(instr)?,
                Instr::I64RemU(instr) => self.execute_i64_rem_u(instr)?,
                Instr::I64RemUImm16(instr) => self.execute_i64_rem_u_imm16(instr),
                Instr::I64RemUImm16Rev(instr) => self.execute_i64_rem_u_imm16_rev(instr)?,
                Instr::I64And(instr) => self.execute_i64_and(instr),
                Instr::I64AndImm16(instr) => self.execute_i64_and_imm16(instr),
                Instr::I64Or(instr) => self.execute_i64_or(instr),
                Instr::I64OrImm16(instr) => self.execute_i64_or_imm16(instr),
                Instr::I64Xor(instr) => self.execute_i64_xor(instr),
                Instr::I64XorImm16(instr) => self.execute_i64_xor_imm16(instr),
                Instr::I64Shl(instr) => self.execute_i64_shl(instr),
                Instr::I64ShlImm(instr) => self.execute_i64_shl_imm(instr),
                Instr::I64ShlImm16Rev(instr) => self.execute_i64_shl_imm16_rev(instr),
                Instr::I64ShrU(instr) => self.execute_i64_shr_u(instr),
                Instr::I64ShrUImm(instr) => self.execute_i64_shr_u_imm(instr),
                Instr::I64ShrUImm16Rev(instr) => self.execute_i64_shr_u_imm16_rev(instr),
                Instr::I64ShrS(instr) => self.execute_i64_shr_s(instr),
                Instr::I64ShrSImm(instr) => self.execute_i64_shr_s_imm(instr),
                Instr::I64ShrSImm16Rev(instr) => self.execute_i64_shr_s_imm16_rev(instr),
                Instr::I64Rotl(instr) => self.execute_i64_rotl(instr),
                Instr::I64RotlImm(instr) => self.execute_i64_rotl_imm(instr),
                Instr::I64RotlImm16Rev(instr) => self.execute_i64_rotl_imm16_rev(instr),
                Instr::I64Rotr(instr) => self.execute_i64_rotr(instr),
                Instr::I64RotrImm(instr) => self.execute_i64_rotr_imm(instr),
                Instr::I64RotrImm16Rev(instr) => self.execute_i64_rotr_imm16_rev(instr),
                Instr::F32Abs(instr) => self.execute_f32_abs(instr),
                Instr::F32Neg(instr) => self.execute_f32_neg(instr),
                Instr::F32Ceil(instr) => self.execute_f32_ceil(instr),
                Instr::F32Floor(instr) => self.execute_f32_floor(instr),
                Instr::F32Trunc(instr) => self.execute_f32_trunc(instr),
                Instr::F32Nearest(instr) => self.execute_f32_nearest(instr),
                Instr::F32Sqrt(instr) => self.execute_f32_sqrt(instr),
                Instr::F64Abs(instr) => self.execute_f64_abs(instr),
                Instr::F64Neg(instr) => self.execute_f64_neg(instr),
                Instr::F64Ceil(instr) => self.execute_f64_ceil(instr),
                Instr::F64Floor(instr) => self.execute_f64_floor(instr),
                Instr::F64Trunc(instr) => self.execute_f64_trunc(instr),
                Instr::F64Nearest(instr) => self.execute_f64_nearest(instr),
                Instr::F64Sqrt(instr) => self.execute_f64_sqrt(instr),
                Instr::F32Add(instr) => self.execute_f32_add(instr),
                Instr::F32Sub(instr) => self.execute_f32_sub(instr),
                Instr::F32Mul(instr) => self.execute_f32_mul(instr),
                Instr::F32Div(instr) => self.execute_f32_div(instr),
                Instr::F32Min(instr) => self.execute_f32_min(instr),
                Instr::F32Max(instr) => self.execute_f32_max(instr),
                Instr::F32Copysign(instr) => self.execute_f32_copysign(instr),
                Instr::F32CopysignImm(instr) => self.execute_f32_copysign_imm(instr),
                Instr::F64Add(instr) => self.execute_f64_add(instr),
                Instr::F64Sub(instr) => self.execute_f64_sub(instr),
                Instr::F64Mul(instr) => self.execute_f64_mul(instr),
                Instr::F64Div(instr) => self.execute_f64_div(instr),
                Instr::F64Min(instr) => self.execute_f64_min(instr),
                Instr::F64Max(instr) => self.execute_f64_max(instr),
                Instr::F64Copysign(instr) => self.execute_f64_copysign(instr),
                Instr::F64CopysignImm(instr) => self.execute_f64_copysign_imm(instr),
                Instr::I32WrapI64(instr) => self.execute_i32_wrap_i64(instr),
                Instr::I64ExtendI32S(instr) => self.execute_i64_extend_i32_s(instr),
                Instr::I64ExtendI32U(instr) => self.execute_i64_extend_i32_u(instr),
                Instr::I32TruncF32S(instr) => self.execute_i32_trunc_f32_s(instr)?,
                Instr::I32TruncF32U(instr) => self.execute_i32_trunc_f32_u(instr)?,
                Instr::I32TruncF64S(instr) => self.execute_i32_trunc_f64_s(instr)?,
                Instr::I32TruncF64U(instr) => self.execute_i32_trunc_f64_u(instr)?,
                Instr::I64TruncF32S(instr) => self.execute_i64_trunc_f32_s(instr)?,
                Instr::I64TruncF32U(instr) => self.execute_i64_trunc_f32_u(instr)?,
                Instr::I64TruncF64S(instr) => self.execute_i64_trunc_f64_s(instr)?,
                Instr::I64TruncF64U(instr) => self.execute_i64_trunc_f64_u(instr)?,
                Instr::I32TruncSatF32S(instr) => self.execute_i32_trunc_sat_f32_s(instr),
                Instr::I32TruncSatF32U(instr) => self.execute_i32_trunc_sat_f32_u(instr),
                Instr::I32TruncSatF64S(instr) => self.execute_i32_trunc_sat_f64_s(instr),
                Instr::I32TruncSatF64U(instr) => self.execute_i32_trunc_sat_f64_u(instr),
                Instr::I64TruncSatF32S(instr) => self.execute_i64_trunc_sat_f32_s(instr),
                Instr::I64TruncSatF32U(instr) => self.execute_i64_trunc_sat_f32_u(instr),
                Instr::I64TruncSatF64S(instr) => self.execute_i64_trunc_sat_f64_s(instr),
                Instr::I64TruncSatF64U(instr) => self.execute_i64_trunc_sat_f64_u(instr),
                Instr::I32Extend8S(instr) => self.execute_i32_extend8_s(instr),
                Instr::I32Extend16S(instr) => self.execute_i32_extend16_s(instr),
                Instr::I64Extend8S(instr) => self.execute_i64_extend8_s(instr),
                Instr::I64Extend16S(instr) => self.execute_i64_extend16_s(instr),
                Instr::I64Extend32S(instr) => self.execute_i64_extend32_s(instr),
                Instr::F32DemoteF64(instr) => self.execute_f32_demote_f64(instr),
                Instr::F64PromoteF32(instr) => self.execute_f64_promote_f32(instr),
                Instr::F32ConvertI32S(instr) => self.execute_f32_convert_i32_s(instr),
                Instr::F32ConvertI32U(instr) => self.execute_f32_convert_i32_u(instr),
                Instr::F32ConvertI64S(instr) => self.execute_f32_convert_i64_s(instr),
                Instr::F32ConvertI64U(instr) => self.execute_f32_convert_i64_u(instr),
                Instr::F64ConvertI32S(instr) => self.execute_f64_convert_i32_s(instr),
                Instr::F64ConvertI32U(instr) => self.execute_f64_convert_i32_u(instr),
                Instr::F64ConvertI64S(instr) => self.execute_f64_convert_i64_s(instr),
                Instr::F64ConvertI64U(instr) => self.execute_f64_convert_i64_u(instr),
            }
        }
    }

    /// Returns the [`Register`] value.
    fn get_register(&self, register: Register) -> UntypedValue {
        // Safety: TODO
        unsafe { self.sp.get(register) }
    }

    /// Returns the [`Register`] value.
    fn get_register_as<T>(&self, register: Register) -> T
    where
        T: From<UntypedValue>,
    {
        T::from(self.get_register(register))
    }

    /// Sets the [`Register`] value to `value`.
    fn set_register(&mut self, register: Register, value: impl Into<UntypedValue>) {
        // Safety: TODO
        unsafe { self.sp.set(register, value.into()) };
    }

    /// Shifts the instruction pointer to the next instruction.
    #[inline(always)]
    fn next_instr(&mut self) {
        self.next_instr_at(1)
    }

    /// Shifts the instruction pointer to the next instruction.
    ///
    /// Has a parameter `skip` to denote how many instruction words
    /// to skip to reach the next actual instruction.
    ///
    /// # Note
    ///
    /// This is used by Wasmi instructions that have a fixed
    /// encoding size of two instruction words such as [`Instruction::Branch`].
    #[inline(always)]
    fn next_instr_at(&mut self, skip: usize) {
        self.ip.add(skip)
    }

    /// Shifts the instruction pointer to the next instruction and returns `Ok(())`.
    ///
    /// # Note
    ///
    /// This is a convenience function for fallible instructions.
    #[inline(always)]
    fn try_next_instr(&mut self) -> Result<(), Error> {
        self.try_next_instr_at(1)
    }

    /// Shifts the instruction pointer to the next instruction and returns `Ok(())`.
    ///
    /// Has a parameter `skip` to denote how many instruction words
    /// to skip to reach the next actual instruction.
    ///
    /// # Note
    ///
    /// This is a convenience function for fallible instructions.
    #[inline(always)]
    fn try_next_instr_at(&mut self, skip: usize) -> Result<(), Error> {
        self.next_instr_at(skip);
        Ok(())
    }

    /// Returns the [`FrameRegisters`] of the [`CallFrame`].
    #[inline]
    fn frame_stack_ptr(&mut self, frame: &CallFrame) -> FrameRegisters {
        Self::frame_stack_ptr_impl(self.value_stack, frame)
    }

    /// Returns the [`FrameRegisters`] of the [`CallFrame`].
    fn frame_stack_ptr_impl(value_stack: &mut ValueStack, frame: &CallFrame) -> FrameRegisters {
        // Safety: We are using the frame's own base offset as input because it is
        //         guaranteed by the Wasm validation and translation phase to be
        //         valid for all register indices used by the associated function body.
        unsafe { value_stack.stack_ptr_at(frame.base_offset()) }
    }

    /// Initializes the [`Executor`] state for the [`CallFrame`].
    ///
    /// # Note
    ///
    /// The initialization of the [`Executor`] allows for efficient execution.
    fn init_call_frame(&mut self, frame: &CallFrame) {
        Self::init_call_frame_impl(
            self.value_stack,
            &mut self.sp,
            &mut self.ip,
            self.cache,
            frame,
        )
    }

    /// Initializes the [`Executor`] state for the [`CallFrame`].
    ///
    /// # Note
    ///
    /// The initialization of the [`Executor`] allows for efficient execution.
    fn init_call_frame_impl(
        value_stack: &mut ValueStack,
        sp: &mut FrameRegisters,
        ip: &mut InstructionPtr,
        cache: &mut InstanceCache,
        frame: &CallFrame,
    ) {
        *sp = Self::frame_stack_ptr_impl(value_stack, frame);
        *ip = frame.instr_ptr();
        cache.update_instance(frame.instance());
    }

    /// Returns the [`Instruction::Const32`] parameter for an [`Instruction`].
    fn fetch_const32(&self, offset: usize) -> AnyConst32 {
        let mut addr: InstructionPtr = self.ip;
        addr.add(offset);
        match *addr.get() {
            Instruction::Const32(value) => value,
            _ => unreachable!("expected an Instruction::Const32 instruction word"),
        }
    }

    /// Returns the [`Instruction::Const32`] parameter for an [`Instruction`].
    fn fetch_address_offset(&self, offset: usize) -> u32 {
        u32::from(self.fetch_const32(offset))
    }

    /// Executes a generic unary [`Instruction`].
    fn execute_unary(&mut self, instr: UnaryInstr, op: fn(UntypedValue) -> UntypedValue) {
        let value = self.get_register(instr.input);
        if self.ctx.engine().config().get_update_runtime_signature() {
            self.update_runtime_signature(value.to_bits());
        }
        self.set_register(instr.result, op(value));
        self.next_instr();
    }

    /// Executes a fallible generic unary [`Instruction`].
    fn try_execute_unary(
        &mut self,
        instr: UnaryInstr,
        op: fn(UntypedValue) -> Result<UntypedValue, TrapCode>,
    ) -> Result<(), Error> {
        let value = self.get_register(instr.input);
        if self.ctx.engine().config().get_update_runtime_signature() {
            self.update_runtime_signature(value.to_bits());
        }
        self.set_register(instr.result, op(value)?);
        self.try_next_instr()
    }

    /// Executes a generic binary [`Instruction`].
    fn execute_binary(
        &mut self,
        instr: BinInstr,
        op: fn(UntypedValue, UntypedValue) -> UntypedValue,
    ) {
        let lhs = self.get_register(instr.lhs);
        let rhs = self.get_register(instr.rhs);
        if self.ctx.engine().config().get_update_runtime_signature() {
            self.update_runtime_signature(lhs.to_bits());
            self.update_runtime_signature(rhs.to_bits());
        }
        self.set_register(instr.result, op(lhs, rhs));
        self.next_instr();
    }

    /// Executes a generic binary [`Instruction`].
    fn execute_binary_imm16<T>(
        &mut self,
        instr: BinInstrImm16<T>,
        op: fn(UntypedValue, UntypedValue) -> UntypedValue,
    ) where
        T: From<Const16<T>>,
        UntypedValue: From<T>,
    {
        let lhs = self.get_register(instr.reg_in);
        let rhs = UntypedValue::from(<T>::from(instr.imm_in));
        if self.ctx.engine().config().get_update_runtime_signature() {
            self.update_runtime_signature(lhs.to_bits());
            self.update_runtime_signature(rhs.to_bits());
        }
        self.set_register(instr.result, op(lhs, rhs));
        self.next_instr();
    }

    /// Executes a generic binary [`Instruction`] with reversed operands.
    fn execute_binary_imm16_rev<T>(
        &mut self,
        instr: BinInstrImm16<T>,
        op: fn(UntypedValue, UntypedValue) -> UntypedValue,
    ) where
        T: From<Const16<T>>,
        UntypedValue: From<T>,
    {
        let lhs = UntypedValue::from(<T>::from(instr.imm_in));
        let rhs = self.get_register(instr.reg_in);
        if self.ctx.engine().config().get_update_runtime_signature() {
            self.update_runtime_signature(lhs.to_bits());
            self.update_runtime_signature(rhs.to_bits());
        }
        self.set_register(instr.result, op(lhs, rhs));
        self.next_instr();
    }

    /// Executes a fallible generic binary [`Instruction`].
    fn try_execute_binary(
        &mut self,
        instr: BinInstr,
        op: fn(UntypedValue, UntypedValue) -> Result<UntypedValue, TrapCode>,
    ) -> Result<(), Error> {
        let lhs = self.get_register(instr.lhs);
        let rhs = self.get_register(instr.rhs);
        if self.ctx.engine().config().get_update_runtime_signature() {
            self.update_runtime_signature(lhs.to_bits());
            self.update_runtime_signature(rhs.to_bits());
        }
        self.set_register(instr.result, op(lhs, rhs)?);
        self.try_next_instr()
    }

    /// Executes a fallible generic binary [`Instruction`].
    fn try_execute_divrem_imm16<NonZeroT>(
        &mut self,
        instr: BinInstrImm16<NonZeroT>,
        op: fn(UntypedValue, NonZeroT) -> Result<UntypedValue, Error>,
    ) -> Result<(), Error>
    where
        NonZeroT: From<Const16<NonZeroT>>,
    {
        let lhs = self.get_register(instr.reg_in);
        let rhs = <NonZeroT>::from(instr.imm_in);
        if self.ctx.engine().config().get_update_runtime_signature() {
            self.update_runtime_signature(lhs.to_bits());
        }
        self.set_register(instr.result, op(lhs, rhs)?);
        self.try_next_instr()
    }

    /// Executes a fallible generic binary [`Instruction`].
    fn execute_divrem_imm16<NonZeroT>(
        &mut self,
        instr: BinInstrImm16<NonZeroT>,
        op: fn(UntypedValue, NonZeroT) -> UntypedValue,
    ) where
        NonZeroT: From<Const16<NonZeroT>>,
    {
        let lhs = self.get_register(instr.reg_in);
        let rhs = <NonZeroT>::from(instr.imm_in);
        if self.ctx.engine().config().get_update_runtime_signature() {
            self.update_runtime_signature(lhs.to_bits());
        }
        self.set_register(instr.result, op(lhs, rhs));
        self.next_instr()
    }

    /// Executes a fallible generic binary [`Instruction`] with reversed operands.
    fn try_execute_binary_imm16_rev<T>(
        &mut self,
        instr: BinInstrImm16<T>,
        op: fn(UntypedValue, UntypedValue) -> Result<UntypedValue, TrapCode>,
    ) -> Result<(), Error>
    where
        T: From<Const16<T>>,
        UntypedValue: From<T>,
    {
        let lhs = UntypedValue::from(<T>::from(instr.imm_in));
        let rhs = self.get_register(instr.reg_in);
        if self.ctx.engine().config().get_update_runtime_signature() {
            self.update_runtime_signature(lhs.to_bits());
            self.update_runtime_signature(rhs.to_bits());
        }
        self.set_register(instr.result, op(lhs, rhs)?);
        self.try_next_instr()
    }

    /// Updates the runtime signature in a fast unpredictable way.
    /// We don't use hashes because we only need unpredictability, not cryptographic security.
    fn update_runtime_signature(&mut self, value: u64) {
        let mut runtime_signature = self.ctx.get_runtime_signature() ^ value;
        runtime_signature ^= runtime_signature >> 27;
        runtime_signature ^= runtime_signature << 23;
        runtime_signature = runtime_signature.wrapping_mul(0xdfd951778ea84a0f);
        self.ctx.set_runtime_signature(runtime_signature);
    }
}

impl<'ctx, 'engine> Executor<'ctx, 'engine> {
    /// Used for all [`Instruction`] words that are not meant for execution.
    ///
    /// # Note
    ///
    /// This includes [`Instruction`] variants such as [`Instruction::TableIdx`]
    /// that primarily carry parameters for actually executable [`Instruction`].
    #[inline(always)]
    fn invalid_instruction_word(&mut self) -> Result<(), Error> {
        self.execute_trap(TrapCode::UnreachableCodeReached)
    }

    /// Executes a Wasm `unreachable` instruction.
    #[inline(always)]
    fn execute_trap(&mut self, trap_code: TrapCode) -> Result<(), Error> {
        Err(Error::from(trap_code))
    }

    /// Executes an [`Instruction::ConsumeFuel`].
    #[inline(always)]
    fn execute_consume_fuel(&mut self, block_fuel: BlockFuel) -> Result<(), Error> {
        // We do not have to check if fuel metering is enabled since
        // [`Instruction::ConsumeFuel`] are only generated if fuel metering
        // is enabled to begin with.
        self.ctx
            .fuel_mut()
            .consume_fuel_unchecked(block_fuel.to_u64())?;
        self.try_next_instr()
    }

    /// Executes an [`Instruction::RefFunc`].
    #[inline(always)]
    fn execute_ref_func(&mut self, result: Register, func_index: FuncIdx) {
        let func = self.cache.get_func(self.ctx, func_index);
        let funcref = FuncRef::new(func);
        self.set_register(result, funcref);
        self.next_instr();
    }
}

/// Extension method for [`UntypedValue`] required by the [`Executor`].
trait UntypedValueExt {
    /// Executes a fused `i32.and` + `i32.eqz` instruction.
    fn i32_and_eqz(x: UntypedValue, y: UntypedValue) -> UntypedValue;

    /// Executes a fused `i32.or` + `i32.eqz` instruction.
    fn i32_or_eqz(x: UntypedValue, y: UntypedValue) -> UntypedValue;

    /// Executes a fused `i32.xor` + `i32.eqz` instruction.
    fn i32_xor_eqz(x: UntypedValue, y: UntypedValue) -> UntypedValue;
}

impl UntypedValueExt for UntypedValue {
    fn i32_and_eqz(x: UntypedValue, y: UntypedValue) -> UntypedValue {
        (i32::from(UntypedValue::i32_and(x, y)) == 0).into()
    }

    fn i32_or_eqz(x: UntypedValue, y: UntypedValue) -> UntypedValue {
        (i32::from(UntypedValue::i32_or(x, y)) == 0).into()
    }

    fn i32_xor_eqz(x: UntypedValue, y: UntypedValue) -> UntypedValue {
        (i32::from(UntypedValue::i32_xor(x, y)) == 0).into()
    }
}
