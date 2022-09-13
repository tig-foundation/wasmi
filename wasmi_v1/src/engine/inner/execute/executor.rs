use super::{cache::InstanceCache, stack::StackFrameView, CallOutcome};
use crate::{
    engine::{
        bytecode::{self, ExecRegister, ExecuteTypes},
        code_map::{CodeMap, ResolvedFuncBody},
        inner::EngineResources,
        ConstRef,
        ExecProvider,
        ExecProviderSlice,
        ExecRegisterSlice,
        InstructionTypes,
        Target,
    },
    module::{FuncIdx, FuncTypeIdx},
    AsContextMut,
    Func,
    Memory,
    StoreContextMut,
    Table,
};
use bytecode::ExecInstruction;
use core::cmp;
use wasmi_core::{
    memory_units::Pages,
    ExtendInto,
    LittleEndianConvert,
    Trap,
    TrapCode,
    UntypedValue,
    WrapInto,
    F32,
    F64,
};

/// The result of a conditional return with a single return value.
#[derive(Debug, Copy, Clone)]
pub enum ConditionalReturn {
    /// Continue with the next instruction.
    Continue,
    /// Return control back to the caller of the function.
    ///
    /// Returning a single result value.
    Return { result: UntypedValue },
}

/// The result of a conditional return with any number of return values.
#[derive(Debug, Copy, Clone)]
pub enum ConditionalReturnMulti {
    /// Continue with the next instruction.
    Continue,
    /// Return control back to the caller of the function.
    ///
    /// Returning any number of result values.
    Return { results: ExecProviderSlice },
}

/// Executes the given [`StackFrameView`].
///
/// Returns the outcome of the execution.
///
/// # Errors
///
/// If the execution traps.
///
/// # Panics
///
/// If resources are missing unexpectedly.
/// For example, a linear memory instance, global variable, etc.
#[inline(always)]
pub(super) fn execute_frame(
    mut ctx: impl AsContextMut,
    code_map: &CodeMap,
    res: &EngineResources,
    frame: StackFrameView,
    cache: &mut InstanceCache,
) -> Result<CallOutcome, Trap> {
    Executor::new(ctx.as_context_mut(), code_map, res, frame, cache).execute()
}

/// An executor to execute a single function frame until it is done.
#[derive(Debug)]
pub struct Executor<'engine, 'func, 'ctx, 'cache, T> {
    /// The program counter.
    ///
    /// # Note
    ///
    /// We carved the `pc` out of `frame` to make it more cache friendly.
    /// Upon returning to the caller we will update the frame's `pc` to
    /// keep it in sync.
    pc: usize,
    /// The function frame that is being executed.
    frame: StackFrameView<'func>,
    /// The read-only engine resources.
    res: &'engine EngineResources,
    /// The associated store context.
    ctx: StoreContextMut<'ctx, T>,
    /// Cache for frequently used instance related entities.
    ///
    /// # Note
    ///
    /// This is mainly used as a cache for fast default
    /// linear memory and default table accesses.
    cache: &'cache mut InstanceCache,
    /// The resolved function body.
    func_body: ResolvedFuncBody<'engine>,
}

impl<'engine, 'func, 'ctx, 'cache, T> Executor<'engine, 'func, 'ctx, 'cache, T> {
    /// Create a new [`Executor`] for the given function `frame`.
    #[inline(always)]
    fn new(
        ctx: StoreContextMut<'ctx, T>,
        code_map: &'engine CodeMap,
        res: &'engine EngineResources,
        frame: StackFrameView<'func>,
        cache: &'cache mut InstanceCache,
    ) -> Self {
        let func_body = code_map.resolve(frame.func_body());
        cache.update_instance(frame.instance());
        let pc = frame.pc();
        Self {
            pc,
            frame,
            res,
            ctx,
            cache,
            func_body,
        }
    }

    /// Returns a shared reference to the next [`ExecInstruction`].
    #[inline]
    fn instr(&self) -> &ExecInstruction {
        // # Safety
        //
        // Since the Wasm and `wasmi` bytecode has already been validated the
        // indices passed at this point can be assumed to be valid always.
        unsafe { self.func_body.get_release_unchecked(self.pc) }
    }

    /// Executes the given function frame until the end.
    #[inline(always)]
    fn execute(mut self) -> Result<CallOutcome, Trap> {
        loop {
            use bytecode::Instruction as Instr;
            match *self.instr() {
                Instr::Br { target } => self.exec_br(target),
                Instr::BrCopy {
                    target,
                    result,
                    returned,
                } => self.exec_br_copy(target, result, returned),
                Instr::BrCopyImm {
                    target,
                    result,
                    returned,
                } => self.exec_br_copy_imm(target, result, returned),
                Instr::BrCopyMulti {
                    results,
                    returned,
                    target,
                } => self.exec_br_copy_multi(target, results, returned),
                Instr::BrEqz { target, condition } => self.exec_br_eqz(target, condition),
                Instr::BrNez { target, condition } => self.exec_br_nez(target, condition),
                Instr::BrNezCopy {
                    result,
                    returned,
                    target,
                    condition,
                } => self.exec_br_nez_copy(target, condition, result, returned),
                Instr::BrNezCopyImm {
                    result,
                    returned,
                    target,
                    condition,
                } => self.exec_br_nez_copy_imm(target, condition, result, returned),
                Instr::BrNezCopyMulti {
                    results,
                    returned,
                    target,
                    condition,
                } => self.exec_br_nez_copy_multi(target, condition, results, returned),
                Instr::ReturnNez { result, condition } => {
                    if let ConditionalReturn::Return { result } =
                        self.exec_return_nez(result, condition)
                    {
                        return Ok(CallOutcome::ReturnSingle { returned: result });
                    }
                }
                Instr::ReturnNezImm { result, condition } => {
                    if let ConditionalReturn::Return { result } =
                        self.exec_return_nez_imm(result, condition)
                    {
                        return Ok(CallOutcome::ReturnSingle { returned: result });
                    }
                }
                Instr::ReturnNezMulti { results, condition } => {
                    if let ConditionalReturnMulti::Return { results } =
                        self.exec_return_nez_multi(results, condition)
                    {
                        return Ok(CallOutcome::ReturnMulti { returned: results });
                    }
                }
                Instr::BrTable { case, len_targets } => self.exec_br_table(case, len_targets),
                Instr::Trap { trap_code } => {
                    self.exec_trap(trap_code)?;
                }
                Instr::Return { result } => return self.exec_return(result),
                Instr::ReturnImm { result } => return self.exec_return_imm(result),
                Instr::ReturnMulti { results } => return self.exec_return_multi(results),
                Instr::Call {
                    func_idx,
                    results,
                    params,
                } => return self.exec_call(func_idx, results, params),
                Instr::CallIndirect {
                    func_type_idx,
                    results,
                    index,
                    params,
                } => return self.exec_call_indirect(func_type_idx, results, index, params),
                Instr::Copy { result, input } => self.exec_copy(result, input),
                Instr::CopyImm { result, input } => self.exec_copy_imm(result, input),
                Instr::CopyMany { results, inputs } => self.exec_copy_many(results, inputs),
                Instr::Select {
                    result,
                    condition,
                    if_true,
                    if_false,
                } => self.exec_select(result, condition, if_true, if_false),
                Instr::GlobalGet { result, global } => self.exec_global_get(result, global),
                Instr::GlobalSet { global, value } => self.exec_global_set(global, value),
                Instr::GlobalSetImm { global, value } => self.exec_global_set_imm(global, value),
                Instr::I32Load {
                    result,
                    ptr,
                    offset,
                } => {
                    self.exec_i32_load(result, ptr, offset)?;
                }
                Instr::I64Load {
                    result,
                    ptr,
                    offset,
                } => {
                    self.exec_i64_load(result, ptr, offset)?;
                }
                Instr::F32Load {
                    result,
                    ptr,
                    offset,
                } => {
                    self.exec_f32_load(result, ptr, offset)?;
                }
                Instr::F64Load {
                    result,
                    ptr,
                    offset,
                } => {
                    self.exec_f64_load(result, ptr, offset)?;
                }
                Instr::I32Load8S {
                    result,
                    ptr,
                    offset,
                } => {
                    self.exec_i32_load_8_s(result, ptr, offset)?;
                }
                Instr::I32Load8U {
                    result,
                    ptr,
                    offset,
                } => {
                    self.exec_i32_load_8_u(result, ptr, offset)?;
                }
                Instr::I32Load16S {
                    result,
                    ptr,
                    offset,
                } => {
                    self.exec_i32_load_16_s(result, ptr, offset)?;
                }
                Instr::I32Load16U {
                    result,
                    ptr,
                    offset,
                } => {
                    self.exec_i32_load_16_u(result, ptr, offset)?;
                }
                Instr::I64Load8S {
                    result,
                    ptr,
                    offset,
                } => {
                    self.exec_i64_load_8_s(result, ptr, offset)?;
                }
                Instr::I64Load8U {
                    result,
                    ptr,
                    offset,
                } => {
                    self.exec_i64_load_8_u(result, ptr, offset)?;
                }
                Instr::I64Load16S {
                    result,
                    ptr,
                    offset,
                } => {
                    self.exec_i64_load_16_s(result, ptr, offset)?;
                }
                Instr::I64Load16U {
                    result,
                    ptr,
                    offset,
                } => {
                    self.exec_i64_load_16_u(result, ptr, offset)?;
                }
                Instr::I64Load32S {
                    result,
                    ptr,
                    offset,
                } => {
                    self.exec_i64_load_32_s(result, ptr, offset)?;
                }
                Instr::I64Load32U {
                    result,
                    ptr,
                    offset,
                } => {
                    self.exec_i64_load_32_u(result, ptr, offset)?;
                }
                Instr::I32Store { ptr, offset, value } => {
                    self.exec_i32_store(ptr, offset, value)?;
                }
                Instr::I32StoreImm { ptr, offset, value } => {
                    self.exec_i32_store_imm(ptr, offset, value)?;
                }
                Instr::I64Store { ptr, offset, value } => {
                    self.exec_i64_store(ptr, offset, value)?;
                }
                Instr::I64StoreImm { ptr, offset, value } => {
                    self.exec_i64_store_imm(ptr, offset, value)?;
                }
                Instr::F32Store { ptr, offset, value } => {
                    self.exec_f32_store(ptr, offset, value)?;
                }
                Instr::F32StoreImm { ptr, offset, value } => {
                    self.exec_f32_store_imm(ptr, offset, value)?;
                }
                Instr::F64Store { ptr, offset, value } => {
                    self.exec_f64_store(ptr, offset, value)?;
                }
                Instr::F64StoreImm { ptr, offset, value } => {
                    self.exec_f64_store_imm(ptr, offset, value)?;
                }
                Instr::I32Store8 { ptr, offset, value } => {
                    self.exec_i32_store_8(ptr, offset, value)?;
                }
                Instr::I32Store8Imm { ptr, offset, value } => {
                    self.exec_i32_store_8_imm(ptr, offset, value)?;
                }
                Instr::I32Store16 { ptr, offset, value } => {
                    self.exec_i32_store_16(ptr, offset, value)?;
                }
                Instr::I32Store16Imm { ptr, offset, value } => {
                    self.exec_i32_store_16_imm(ptr, offset, value)?;
                }
                Instr::I64Store8 { ptr, offset, value } => {
                    self.exec_i64_store_8(ptr, offset, value)?;
                }
                Instr::I64Store8Imm { ptr, offset, value } => {
                    self.exec_i64_store_8_imm(ptr, offset, value)?;
                }
                Instr::I64Store16 { ptr, offset, value } => {
                    self.exec_i64_store_16(ptr, offset, value)?;
                }
                Instr::I64Store16Imm { ptr, offset, value } => {
                    self.exec_i64_store_16_imm(ptr, offset, value)?;
                }
                Instr::I64Store32 { ptr, offset, value } => {
                    self.exec_i64_store_32(ptr, offset, value)?;
                }
                Instr::I64Store32Imm { ptr, offset, value } => {
                    self.exec_i64_store_32_imm(ptr, offset, value)?;
                }
                Instr::MemorySize { result } => self.exec_memory_size(result),
                Instr::MemoryGrow { result, amount } => self.exec_memory_grow(result, amount),
                Instr::I32Eq { result, lhs, rhs } => self.exec_i32_eq(result, lhs, rhs),
                Instr::I32EqImm { result, lhs, rhs } => self.exec_i32_eq_imm(result, lhs, rhs),
                Instr::I32Ne { result, lhs, rhs } => self.exec_i32_ne(result, lhs, rhs),
                Instr::I32NeImm { result, lhs, rhs } => self.exec_i32_ne_imm(result, lhs, rhs),
                Instr::I32LtS { result, lhs, rhs } => self.exec_i32_lt_s(result, lhs, rhs),
                Instr::I32LtSImm { result, lhs, rhs } => self.exec_i32_lt_s_imm(result, lhs, rhs),
                Instr::I32LtU { result, lhs, rhs } => self.exec_i32_lt_u(result, lhs, rhs),
                Instr::I32LtUImm { result, lhs, rhs } => self.exec_i32_lt_u_imm(result, lhs, rhs),
                Instr::I32GtS { result, lhs, rhs } => self.exec_i32_gt_s(result, lhs, rhs),
                Instr::I32GtSImm { result, lhs, rhs } => self.exec_i32_gt_s_imm(result, lhs, rhs),
                Instr::I32GtU { result, lhs, rhs } => self.exec_i32_gt_u(result, lhs, rhs),
                Instr::I32GtUImm { result, lhs, rhs } => self.exec_i32_gt_u_imm(result, lhs, rhs),
                Instr::I32LeS { result, lhs, rhs } => self.exec_i32_le_s(result, lhs, rhs),
                Instr::I32LeSImm { result, lhs, rhs } => self.exec_i32_le_s_imm(result, lhs, rhs),
                Instr::I32LeU { result, lhs, rhs } => self.exec_i32_le_u(result, lhs, rhs),
                Instr::I32LeUImm { result, lhs, rhs } => self.exec_i32_le_u_imm(result, lhs, rhs),
                Instr::I32GeS { result, lhs, rhs } => self.exec_i32_ge_s(result, lhs, rhs),
                Instr::I32GeSImm { result, lhs, rhs } => self.exec_i32_ge_s_imm(result, lhs, rhs),
                Instr::I32GeU { result, lhs, rhs } => self.exec_i32_ge_u(result, lhs, rhs),
                Instr::I32GeUImm { result, lhs, rhs } => self.exec_i32_ge_u_imm(result, lhs, rhs),
                Instr::I64Eq { result, lhs, rhs } => self.exec_i64_eq(result, lhs, rhs),
                Instr::I64EqImm { result, lhs, rhs } => self.exec_i64_eq_imm(result, lhs, rhs),
                Instr::I64Ne { result, lhs, rhs } => self.exec_i64_ne(result, lhs, rhs),
                Instr::I64NeImm { result, lhs, rhs } => self.exec_i64_ne_imm(result, lhs, rhs),
                Instr::I64LtS { result, lhs, rhs } => self.exec_i64_lt_s(result, lhs, rhs),
                Instr::I64LtSImm { result, lhs, rhs } => self.exec_i64_lt_s_imm(result, lhs, rhs),
                Instr::I64LtU { result, lhs, rhs } => self.exec_i64_lt_u(result, lhs, rhs),
                Instr::I64LtUImm { result, lhs, rhs } => self.exec_i64_lt_u_imm(result, lhs, rhs),
                Instr::I64GtS { result, lhs, rhs } => self.exec_i64_gt_s(result, lhs, rhs),
                Instr::I64GtSImm { result, lhs, rhs } => self.exec_i64_gt_s_imm(result, lhs, rhs),
                Instr::I64GtU { result, lhs, rhs } => self.exec_i64_gt_u(result, lhs, rhs),
                Instr::I64GtUImm { result, lhs, rhs } => self.exec_i64_gt_u_imm(result, lhs, rhs),
                Instr::I64LeS { result, lhs, rhs } => self.exec_i64_le_s(result, lhs, rhs),
                Instr::I64LeSImm { result, lhs, rhs } => self.exec_i64_le_s_imm(result, lhs, rhs),
                Instr::I64LeU { result, lhs, rhs } => self.exec_i64_le_u(result, lhs, rhs),
                Instr::I64LeUImm { result, lhs, rhs } => self.exec_i64_le_u_imm(result, lhs, rhs),
                Instr::I64GeS { result, lhs, rhs } => self.exec_i64_ge_s(result, lhs, rhs),
                Instr::I64GeSImm { result, lhs, rhs } => self.exec_i64_ge_s_imm(result, lhs, rhs),
                Instr::I64GeU { result, lhs, rhs } => self.exec_i64_ge_u(result, lhs, rhs),
                Instr::I64GeUImm { result, lhs, rhs } => self.exec_i64_ge_u_imm(result, lhs, rhs),
                Instr::F32Eq { result, lhs, rhs } => self.exec_f32_eq(result, lhs, rhs),
                Instr::F32EqImm { result, lhs, rhs } => self.exec_f32_eq_imm(result, lhs, rhs),
                Instr::F32Ne { result, lhs, rhs } => self.exec_f32_ne(result, lhs, rhs),
                Instr::F32NeImm { result, lhs, rhs } => self.exec_f32_ne_imm(result, lhs, rhs),
                Instr::F32Lt { result, lhs, rhs } => self.exec_f32_lt(result, lhs, rhs),
                Instr::F32LtImm { result, lhs, rhs } => self.exec_f32_lt_imm(result, lhs, rhs),
                Instr::F32Gt { result, lhs, rhs } => self.exec_f32_gt(result, lhs, rhs),
                Instr::F32GtImm { result, lhs, rhs } => self.exec_f32_gt_imm(result, lhs, rhs),
                Instr::F32Le { result, lhs, rhs } => self.exec_f32_le(result, lhs, rhs),
                Instr::F32LeImm { result, lhs, rhs } => self.exec_f32_le_imm(result, lhs, rhs),
                Instr::F32Ge { result, lhs, rhs } => self.exec_f32_ge(result, lhs, rhs),
                Instr::F32GeImm { result, lhs, rhs } => self.exec_f32_ge_imm(result, lhs, rhs),
                Instr::F64Eq { result, lhs, rhs } => self.exec_f64_eq(result, lhs, rhs),
                Instr::F64EqImm { result, lhs, rhs } => self.exec_f64_eq_imm(result, lhs, rhs),
                Instr::F64Ne { result, lhs, rhs } => self.exec_f64_ne(result, lhs, rhs),
                Instr::F64NeImm { result, lhs, rhs } => self.exec_f64_ne_imm(result, lhs, rhs),
                Instr::F64Lt { result, lhs, rhs } => self.exec_f64_lt(result, lhs, rhs),
                Instr::F64LtImm { result, lhs, rhs } => self.exec_f64_lt_imm(result, lhs, rhs),
                Instr::F64Gt { result, lhs, rhs } => self.exec_f64_gt(result, lhs, rhs),
                Instr::F64GtImm { result, lhs, rhs } => self.exec_f64_gt_imm(result, lhs, rhs),
                Instr::F64Le { result, lhs, rhs } => self.exec_f64_le(result, lhs, rhs),
                Instr::F64LeImm { result, lhs, rhs } => self.exec_f64_le_imm(result, lhs, rhs),
                Instr::F64Ge { result, lhs, rhs } => self.exec_f64_ge(result, lhs, rhs),
                Instr::F64GeImm { result, lhs, rhs } => self.exec_f64_ge_imm(result, lhs, rhs),
                Instr::I32Clz { result, input } => self.exec_i32_clz(result, input),
                Instr::I32Ctz { result, input } => self.exec_i32_ctz(result, input),
                Instr::I32Popcnt { result, input } => self.exec_i32_popcnt(result, input),
                Instr::I32Add { result, lhs, rhs } => self.exec_i32_add(result, lhs, rhs),
                Instr::I32AddImm { result, lhs, rhs } => self.exec_i32_add_imm(result, lhs, rhs),
                Instr::I32Sub { result, lhs, rhs } => self.exec_i32_sub(result, lhs, rhs),
                Instr::I32SubImm { result, lhs, rhs } => self.exec_i32_sub_imm(result, lhs, rhs),
                Instr::I32Mul { result, lhs, rhs } => self.exec_i32_mul(result, lhs, rhs),
                Instr::I32MulImm { result, lhs, rhs } => self.exec_i32_mul_imm(result, lhs, rhs),
                Instr::I32DivS { result, lhs, rhs } => {
                    self.exec_i32_div_s(result, lhs, rhs)?;
                }
                Instr::I32DivSImm { result, lhs, rhs } => {
                    self.exec_i32_div_s_imm(result, lhs, rhs)?;
                }
                Instr::I32DivU { result, lhs, rhs } => {
                    self.exec_i32_div_u(result, lhs, rhs)?;
                }
                Instr::I32DivUImm { result, lhs, rhs } => {
                    self.exec_i32_div_u_imm(result, lhs, rhs)?;
                }
                Instr::I32RemS { result, lhs, rhs } => {
                    self.exec_i32_rem_s(result, lhs, rhs)?;
                }
                Instr::I32RemSImm { result, lhs, rhs } => {
                    self.exec_i32_rem_s_imm(result, lhs, rhs)?;
                }
                Instr::I32RemU { result, lhs, rhs } => {
                    self.exec_i32_rem_u(result, lhs, rhs)?;
                }
                Instr::I32RemUImm { result, lhs, rhs } => {
                    self.exec_i32_rem_u_imm(result, lhs, rhs)?;
                }
                Instr::I32And { result, lhs, rhs } => self.exec_i32_and(result, lhs, rhs),
                Instr::I32AndImm { result, lhs, rhs } => self.exec_i32_and_imm(result, lhs, rhs),
                Instr::I32Or { result, lhs, rhs } => self.exec_i32_or(result, lhs, rhs),
                Instr::I32OrImm { result, lhs, rhs } => self.exec_i32_or_imm(result, lhs, rhs),
                Instr::I32Xor { result, lhs, rhs } => self.exec_i32_xor(result, lhs, rhs),
                Instr::I32XorImm { result, lhs, rhs } => self.exec_i32_xor_imm(result, lhs, rhs),
                Instr::I32Shl { result, lhs, rhs } => self.exec_i32_shl(result, lhs, rhs),
                Instr::I32ShlImm { result, lhs, rhs } => self.exec_i32_shl_imm(result, lhs, rhs),
                Instr::I32ShrS { result, lhs, rhs } => self.exec_i32_shr_s(result, lhs, rhs),
                Instr::I32ShrSImm { result, lhs, rhs } => self.exec_i32_shr_s_imm(result, lhs, rhs),
                Instr::I32ShrU { result, lhs, rhs } => self.exec_i32_shr_u(result, lhs, rhs),
                Instr::I32ShrUImm { result, lhs, rhs } => self.exec_i32_shr_u_imm(result, lhs, rhs),
                Instr::I32Rotl { result, lhs, rhs } => self.exec_i32_rotl(result, lhs, rhs),
                Instr::I32RotlImm { result, lhs, rhs } => self.exec_i32_rotl_imm(result, lhs, rhs),
                Instr::I32Rotr { result, lhs, rhs } => self.exec_i32_rotr(result, lhs, rhs),
                Instr::I32RotrImm { result, lhs, rhs } => self.exec_i32_rotr_imm(result, lhs, rhs),
                Instr::I64Clz { result, input } => self.exec_i64_clz(result, input),
                Instr::I64Ctz { result, input } => self.exec_i64_ctz(result, input),
                Instr::I64Popcnt { result, input } => self.exec_i64_popcnt(result, input),
                Instr::I64Add { result, lhs, rhs } => self.exec_i64_add(result, lhs, rhs),
                Instr::I64AddImm { result, lhs, rhs } => self.exec_i64_add_imm(result, lhs, rhs),
                Instr::I64Sub { result, lhs, rhs } => self.exec_i64_sub(result, lhs, rhs),
                Instr::I64SubImm { result, lhs, rhs } => self.exec_i64_sub_imm(result, lhs, rhs),
                Instr::I64Mul { result, lhs, rhs } => self.exec_i64_mul(result, lhs, rhs),
                Instr::I64MulImm { result, lhs, rhs } => self.exec_i64_mul_imm(result, lhs, rhs),
                Instr::I64DivS { result, lhs, rhs } => {
                    self.exec_i64_div_s(result, lhs, rhs)?;
                }
                Instr::I64DivSImm { result, lhs, rhs } => {
                    self.exec_i64_div_s_imm(result, lhs, rhs)?;
                }
                Instr::I64DivU { result, lhs, rhs } => {
                    self.exec_i64_div_u(result, lhs, rhs)?;
                }
                Instr::I64DivUImm { result, lhs, rhs } => {
                    self.exec_i64_div_u_imm(result, lhs, rhs)?;
                }
                Instr::I64RemS { result, lhs, rhs } => {
                    self.exec_i64_rem_s(result, lhs, rhs)?;
                }
                Instr::I64RemSImm { result, lhs, rhs } => {
                    self.exec_i64_rem_s_imm(result, lhs, rhs)?;
                }
                Instr::I64RemU { result, lhs, rhs } => {
                    self.exec_i64_rem_u(result, lhs, rhs)?;
                }
                Instr::I64RemUImm { result, lhs, rhs } => {
                    self.exec_i64_rem_u_imm(result, lhs, rhs)?;
                }
                Instr::I64And { result, lhs, rhs } => self.exec_i64_and(result, lhs, rhs),
                Instr::I64AndImm { result, lhs, rhs } => self.exec_i64_and_imm(result, lhs, rhs),
                Instr::I64Or { result, lhs, rhs } => self.exec_i64_or(result, lhs, rhs),
                Instr::I64OrImm { result, lhs, rhs } => self.exec_i64_or_imm(result, lhs, rhs),
                Instr::I64Xor { result, lhs, rhs } => self.exec_i64_xor(result, lhs, rhs),
                Instr::I64XorImm { result, lhs, rhs } => self.exec_i64_xor_imm(result, lhs, rhs),
                Instr::I64Shl { result, lhs, rhs } => self.exec_i64_shl(result, lhs, rhs),
                Instr::I64ShlImm { result, lhs, rhs } => self.exec_i64_shl_imm(result, lhs, rhs),
                Instr::I64ShrS { result, lhs, rhs } => self.exec_i64_shr_s(result, lhs, rhs),
                Instr::I64ShrSImm { result, lhs, rhs } => self.exec_i64_shr_s_imm(result, lhs, rhs),
                Instr::I64ShrU { result, lhs, rhs } => self.exec_i64_shr_u(result, lhs, rhs),
                Instr::I64ShrUImm { result, lhs, rhs } => self.exec_i64_shr_u_imm(result, lhs, rhs),
                Instr::I64Rotl { result, lhs, rhs } => self.exec_i64_rotl(result, lhs, rhs),
                Instr::I64RotlImm { result, lhs, rhs } => self.exec_i64_rotl_imm(result, lhs, rhs),
                Instr::I64Rotr { result, lhs, rhs } => self.exec_i64_rotr(result, lhs, rhs),
                Instr::I64RotrImm { result, lhs, rhs } => self.exec_i64_rotr_imm(result, lhs, rhs),
                Instr::F32Abs { result, input } => self.exec_f32_abs(result, input),
                Instr::F32Neg { result, input } => self.exec_f32_neg(result, input),
                Instr::F32Ceil { result, input } => self.exec_f32_ceil(result, input),
                Instr::F32Floor { result, input } => self.exec_f32_floor(result, input),
                Instr::F32Trunc { result, input } => self.exec_f32_trunc(result, input),
                Instr::F32Nearest { result, input } => self.exec_f32_nearest(result, input),
                Instr::F32Sqrt { result, input } => self.exec_f32_sqrt(result, input),
                Instr::F32Add { result, lhs, rhs } => self.exec_f32_add(result, lhs, rhs),
                Instr::F32AddImm { result, lhs, rhs } => self.exec_f32_add_imm(result, lhs, rhs),
                Instr::F32Sub { result, lhs, rhs } => self.exec_f32_sub(result, lhs, rhs),
                Instr::F32SubImm { result, lhs, rhs } => self.exec_f32_sub_imm(result, lhs, rhs),
                Instr::F32Mul { result, lhs, rhs } => self.exec_f32_mul(result, lhs, rhs),
                Instr::F32MulImm { result, lhs, rhs } => self.exec_f32_mul_imm(result, lhs, rhs),
                Instr::F32Div { result, lhs, rhs } => {
                    self.exec_f32_div(result, lhs, rhs)?;
                }
                Instr::F32DivImm { result, lhs, rhs } => {
                    self.exec_f32_div_imm(result, lhs, rhs)?;
                }
                Instr::F32Min { result, lhs, rhs } => self.exec_f32_min(result, lhs, rhs),
                Instr::F32MinImm { result, lhs, rhs } => self.exec_f32_min_imm(result, lhs, rhs),
                Instr::F32Max { result, lhs, rhs } => self.exec_f32_max(result, lhs, rhs),
                Instr::F32MaxImm { result, lhs, rhs } => self.exec_f32_max_imm(result, lhs, rhs),
                Instr::F32Copysign { result, lhs, rhs } => self.exec_f32_copysign(result, lhs, rhs),
                Instr::F32CopysignImm { result, lhs, rhs } => {
                    self.exec_f32_copysign_imm(result, lhs, rhs)
                }
                Instr::F64Abs { result, input } => self.exec_f64_abs(result, input),
                Instr::F64Neg { result, input } => self.exec_f64_neg(result, input),
                Instr::F64Ceil { result, input } => self.exec_f64_ceil(result, input),
                Instr::F64Floor { result, input } => self.exec_f64_floor(result, input),
                Instr::F64Trunc { result, input } => self.exec_f64_trunc(result, input),
                Instr::F64Nearest { result, input } => self.exec_f64_nearest(result, input),
                Instr::F64Sqrt { result, input } => self.exec_f64_sqrt(result, input),
                Instr::F64Add { result, lhs, rhs } => self.exec_f64_add(result, lhs, rhs),
                Instr::F64AddImm { result, lhs, rhs } => self.exec_f64_add_imm(result, lhs, rhs),
                Instr::F64Sub { result, lhs, rhs } => self.exec_f64_sub(result, lhs, rhs),
                Instr::F64SubImm { result, lhs, rhs } => self.exec_f64_sub_imm(result, lhs, rhs),
                Instr::F64Mul { result, lhs, rhs } => self.exec_f64_mul(result, lhs, rhs),
                Instr::F64MulImm { result, lhs, rhs } => self.exec_f64_mul_imm(result, lhs, rhs),
                Instr::F64Div { result, lhs, rhs } => {
                    self.exec_f64_div(result, lhs, rhs)?;
                }
                Instr::F64DivImm { result, lhs, rhs } => {
                    self.exec_f64_div_imm(result, lhs, rhs)?;
                }
                Instr::F64Min { result, lhs, rhs } => self.exec_f64_min(result, lhs, rhs),
                Instr::F64MinImm { result, lhs, rhs } => self.exec_f64_min_imm(result, lhs, rhs),
                Instr::F64Max { result, lhs, rhs } => self.exec_f64_max(result, lhs, rhs),
                Instr::F64MaxImm { result, lhs, rhs } => self.exec_f64_max_imm(result, lhs, rhs),
                Instr::F64Copysign { result, lhs, rhs } => self.exec_f64_copysign(result, lhs, rhs),
                Instr::F64CopysignImm { result, lhs, rhs } => {
                    self.exec_f64_copysign_imm(result, lhs, rhs)
                }
                Instr::I32WrapI64 { result, input } => self.exec_i32_wrap_i64(result, input),
                Instr::I32TruncSF32 { result, input } => {
                    self.exec_i32_trunc_f32_s(result, input)?;
                }
                Instr::I32TruncUF32 { result, input } => {
                    self.exec_i32_trunc_f32_u(result, input)?;
                }
                Instr::I32TruncSF64 { result, input } => {
                    self.exec_i32_trunc_f64_s(result, input)?;
                }
                Instr::I32TruncUF64 { result, input } => {
                    self.exec_i32_trunc_f64_u(result, input)?;
                }
                Instr::I64ExtendSI32 { result, input } => self.exec_i64_extend_i32_s(result, input),
                Instr::I64ExtendUI32 { result, input } => self.exec_i64_extend_i32_u(result, input),
                Instr::I64TruncSF32 { result, input } => {
                    self.exec_i64_trunc_f32_s(result, input)?;
                }
                Instr::I64TruncUF32 { result, input } => {
                    self.exec_i64_trunc_f32_u(result, input)?;
                }
                Instr::I64TruncSF64 { result, input } => {
                    self.exec_i64_trunc_f64_s(result, input)?;
                }
                Instr::I64TruncUF64 { result, input } => {
                    self.exec_i64_trunc_f64_u(result, input)?;
                }
                Instr::F32ConvertSI32 { result, input } => {
                    self.exec_f32_convert_i32_s(result, input)
                }
                Instr::F32ConvertUI32 { result, input } => {
                    self.exec_f32_convert_i32_u(result, input)
                }
                Instr::F32ConvertSI64 { result, input } => {
                    self.exec_f32_convert_i64_s(result, input)
                }
                Instr::F32ConvertUI64 { result, input } => {
                    self.exec_f32_convert_i64_u(result, input)
                }
                Instr::F32DemoteF64 { result, input } => self.exec_f32_demote_f64(result, input),
                Instr::F64ConvertSI32 { result, input } => {
                    self.exec_f64_convert_i32_s(result, input)
                }
                Instr::F64ConvertUI32 { result, input } => {
                    self.exec_f64_convert_i32_u(result, input)
                }
                Instr::F64ConvertSI64 { result, input } => {
                    self.exec_f64_convert_i64_s(result, input)
                }
                Instr::F64ConvertUI64 { result, input } => {
                    self.exec_f64_convert_i64_u(result, input)
                }
                Instr::F64PromoteF32 { result, input } => self.exec_f64_promote_f32(result, input),
                Instr::I32Extend8S { result, input } => self.exec_i32_extend8_s(result, input),
                Instr::I32Extend16S { result, input } => self.exec_i32_extend16_s(result, input),
                Instr::I64Extend8S { result, input } => self.exec_i64_extend8_s(result, input),
                Instr::I64Extend16S { result, input } => self.exec_i64_extend16_s(result, input),
                Instr::I64Extend32S { result, input } => self.exec_i64_extend32_s(result, input),
                Instr::I32TruncSatF32S { result, input } => {
                    self.exec_i32_trunc_sat_f32_s(result, input)
                }
                Instr::I32TruncSatF32U { result, input } => {
                    self.exec_i32_trunc_sat_f32_u(result, input)
                }
                Instr::I32TruncSatF64S { result, input } => {
                    self.exec_i32_trunc_sat_f64_s(result, input)
                }
                Instr::I32TruncSatF64U { result, input } => {
                    self.exec_i32_trunc_sat_f64_u(result, input)
                }
                Instr::I64TruncSatF32S { result, input } => {
                    self.exec_i64_trunc_sat_f32_s(result, input)
                }
                Instr::I64TruncSatF32U { result, input } => {
                    self.exec_i64_trunc_sat_f32_u(result, input)
                }
                Instr::I64TruncSatF64S { result, input } => {
                    self.exec_i64_trunc_sat_f64_s(result, input)
                }
                Instr::I64TruncSatF64U { result, input } => {
                    self.exec_i64_trunc_sat_f64_u(result, input)
                }
            };
        }
    }

    /// Modifies the `pc` to continue to the next instruction.
    fn next_instr(&mut self) {
        self.pc += 1;
    }

    /// Modifies the `pc` to branches to the given `target`.
    fn branch_to_target(&mut self, target: Target) {
        self.pc = target.destination().into_inner() as usize;
    }

    /// Returns the [`CallOutcome`] to call to the given function.
    ///
    /// # Note
    ///
    /// This is a convenience function with the purpose to simplify
    /// the process to change the behavior of the dispatch once required
    /// for optimization purposes.
    fn call_func(
        &mut self,
        callee: Func,
        results: ExecRegisterSlice,
        params: ExecProviderSlice,
    ) -> Result<CallOutcome, Trap> {
        self.pc += 1;
        self.frame.update_pc(self.pc);
        Ok(CallOutcome::Call {
            callee,
            results,
            params,
        })
    }

    /// Copys values from `src` to `dst`.
    ///
    /// # Panics (Debug)
    ///
    /// If both slices do not have the same length.
    fn copy_many(&mut self, dst: ExecRegisterSlice, src: ExecProviderSlice) {
        debug_assert_eq!(dst.len(), src.len());
        let src = self.res.provider_pool.resolve(src);
        dst.into_iter().zip(src).for_each(|(dst, src)| {
            let src = self.load_provider(*src);
            self.set_register(dst, src);
        });
    }

    /// Returns the default linear memory.
    ///
    /// # Panics
    ///
    /// If there exists is no linear memory for the instance.
    #[inline]
    fn default_memory(&mut self) -> Memory {
        self.cache.default_memory(&self.ctx)
    }

    /// Returns the default table.
    ///
    /// # Panics
    ///
    /// If there exists is no table for the instance.
    #[inline]
    fn default_table(&mut self) -> Table {
        self.cache.default_table(&self.ctx)
    }

    /// Loads the value of the given [`ConstRef`].
    ///
    /// # Panics (Debug)
    ///
    /// If the constant pool does not inhabit the given [`ConstRef`].
    #[inline]
    fn resolve_cref(&self, cref: ConstRef) -> UntypedValue {
        // Safety: We can safely assume that all const references at this
        //         point are valid since we have validated them during
        //         Wasm compilation and validation phase as well as during
        //         wasmi bytecode construction.
        unsafe { self.res.const_pool.resolve_unchecked(cref) }
    }

    /// Returns the global variable at the given index.
    ///
    /// # Panics
    ///
    /// If there is no global variable at the given index.
    #[inline]
    fn resolve_global(&mut self, global_index: bytecode::Global) -> &mut UntypedValue {
        self.cache
            .get_global(self.ctx.as_context_mut(), global_index.into_inner())
    }

    /// Calculates the effective address of a linear memory access.
    ///
    /// # Errors
    ///
    /// If the resulting effective address overflows.
    #[inline]
    fn effective_address(offset: bytecode::Offset, ptr: UntypedValue) -> Result<usize, TrapCode> {
        offset
            .into_inner()
            .checked_add(u32::from(ptr))
            .map(|address| address as usize)
            .ok_or(TrapCode::MemoryAccessOutOfBounds)
    }

    /// Returns the value of the `register`.
    #[inline]
    fn get_register(&self, register: ExecRegister) -> UntypedValue {
        self.frame.regs.get(register)
    }

    /// Sets the value of the `register` to `new_value`.
    #[inline]
    fn set_register(&mut self, register: ExecRegister, new_value: UntypedValue) {
        self.frame.regs.set(register, new_value)
    }

    /// Loads bytes from the default memory into the given `buffer`.
    ///
    /// # Errors
    ///
    /// If the memory access is out of bounds.
    ///
    /// # Panics
    ///
    /// If there exists is no linear memory for the instance.
    fn load_bytes(
        &mut self,
        ptr: ExecRegister,
        offset: bytecode::Offset,
        buffer: &mut [u8],
    ) -> Result<(), TrapCode> {
        let ptr = self.get_register(ptr);
        let address = Self::effective_address(offset, ptr)?;
        self.cache
            .default_memory_bytes(self.ctx.as_context_mut())
            .read(address, buffer)?;
        Ok(())
    }

    /// Stores bytes to the default memory from the given `buffer`.
    ///
    /// # Errors
    ///
    /// If the memory access is out of bounds.
    ///
    /// # Panics
    ///
    /// If there exists is no linear memory for the instance.
    fn store_bytes(
        &mut self,
        ptr: ExecRegister,
        offset: bytecode::Offset,
        bytes: &[u8],
    ) -> Result<(), TrapCode> {
        let ptr = self.get_register(ptr);
        let address = Self::effective_address(offset, ptr)?;
        self.cache
            .default_memory_bytes(self.ctx.as_context_mut())
            .write(address, bytes)?;
        Ok(())
    }

    /// Loads a value of type `T` from the default memory at the given address offset.
    ///
    /// # Note
    ///
    /// This can be used to emulate the following Wasm operands:
    ///
    /// - `i32.load`
    /// - `i64.load`
    /// - `f32.load`
    /// - `f64.load`
    fn exec_load<V>(
        &mut self,
        result: ExecRegister,
        ptr: ExecRegister,
        offset: bytecode::Offset,
    ) -> Result<(), Trap>
    where
        V: LittleEndianConvert + Into<UntypedValue>,
    {
        let mut buffer = <<V as LittleEndianConvert>::Bytes as Default>::default();
        self.load_bytes(ptr, offset, buffer.as_mut())?;
        let value = <V as LittleEndianConvert>::from_le_bytes(buffer);
        self.set_register(result, value.into());
        self.next_instr();
        Ok(())
    }

    /// Loads a vaoue of type `U` from the default memory at the given address offset and extends it into `T`.
    ///
    /// # Note
    ///
    /// This can be used to emuate the following Wasm operands:
    ///
    /// - `i32.load_8s`
    /// - `i32.load_8u`
    /// - `i32.load_16s`
    /// - `i32.load_16u`
    /// - `i64.load_8s`
    /// - `i64.load_8u`
    /// - `i64.load_16s`
    /// - `i64.load_16u`
    /// - `i64.load_32s`
    /// - `i64.load_32u`
    fn exec_load_extend<V, U>(
        &mut self,
        result: ExecRegister,
        ptr: ExecRegister,
        offset: bytecode::Offset,
    ) -> Result<(), Trap>
    where
        V: ExtendInto<U> + LittleEndianConvert,
        U: Into<UntypedValue>,
    {
        let mut buffer = <<V as LittleEndianConvert>::Bytes as Default>::default();
        self.load_bytes(ptr, offset, buffer.as_mut())?;
        let extended = <V as LittleEndianConvert>::from_le_bytes(buffer).extend_into();
        self.set_register(result, extended.into());
        self.next_instr();
        Ok(())
    }

    /// Stores a value of type `T` into the default memory at the given address offset.
    ///
    /// # Note
    ///
    /// This can be used to emulate the following Wasm operands:
    ///
    /// - `i32.store`
    /// - `i64.store`
    /// - `f32.store`
    /// - `f64.store`
    fn exec_store<V>(
        &mut self,
        ptr: ExecRegister,
        offset: bytecode::Offset,
        new_value: ExecRegister,
    ) -> Result<(), Trap>
    where
        V: LittleEndianConvert + From<UntypedValue>,
    {
        let new_value = V::from(self.get_register(new_value));
        let bytes = <V as LittleEndianConvert>::into_le_bytes(new_value);
        self.store_bytes(ptr, offset, bytes.as_ref())?;
        self.next_instr();
        Ok(())
    }

    /// Stores a value of type `T` into the default memory at the given address offset.
    ///
    /// # Note
    ///
    /// This can be used to emulate the following Wasm operands:
    ///
    /// - `i32.store`
    /// - `i64.store`
    /// - `f32.store`
    /// - `f64.store`
    fn exec_store_imm<V>(
        &mut self,
        ptr: ExecRegister,
        offset: bytecode::Offset,
        new_value: UntypedValue,
    ) -> Result<(), Trap>
    where
        V: LittleEndianConvert + From<UntypedValue>,
    {
        let new_value = V::from(new_value);
        let bytes = <V as LittleEndianConvert>::into_le_bytes(new_value);
        self.store_bytes(ptr, offset, bytes.as_ref())?;
        self.next_instr();
        Ok(())
    }

    /// Stores a value of type `T` wrapped to type `U` into the default memory at the given address offset.
    ///
    /// # Note
    ///
    /// This can be used to emulate the following Wasm operands:
    ///
    /// - `i32.store8`
    /// - `i32.store16`
    /// - `i64.store8`
    /// - `i64.store16`
    /// - `i64.store32`
    fn exec_store_wrap<V, U>(
        &mut self,
        ptr: ExecRegister,
        offset: bytecode::Offset,
        new_value: ExecRegister,
    ) -> Result<(), Trap>
    where
        V: From<UntypedValue> + WrapInto<U>,
        U: LittleEndianConvert,
    {
        let new_value = V::from(self.get_register(new_value)).wrap_into();
        let bytes = <U as LittleEndianConvert>::into_le_bytes(new_value);
        self.store_bytes(ptr, offset, bytes.as_ref())?;
        self.next_instr();
        Ok(())
    }

    /// Stores a value of type `T` wrapped to type `U` into the default memory at the given address offset.
    ///
    /// # Note
    ///
    /// This can be used to emulate the following Wasm operands:
    ///
    /// - `i32.store8`
    /// - `i32.store16`
    /// - `i64.store8`
    /// - `i64.store16`
    /// - `i64.store32`
    fn exec_store_wrap_imm<V, U>(
        &mut self,
        ptr: ExecRegister,
        offset: bytecode::Offset,
        new_value: UntypedValue,
    ) -> Result<(), Trap>
    where
        V: From<UntypedValue> + WrapInto<U>,
        U: LittleEndianConvert,
    {
        let new_value = V::from(new_value).wrap_into();
        let bytes = <U as LittleEndianConvert>::into_le_bytes(new_value);
        self.store_bytes(ptr, offset, bytes.as_ref())?;
        self.next_instr();
        Ok(())
    }

    /// Executes the given unary `wasmi` operation.
    ///
    /// # Note
    ///
    /// Loads from the given `input` register,
    /// performs the given operation `op` and stores the
    /// result back into the `result` register.
    ///
    /// # Errors
    ///
    /// Returns `Result::Ok` for convenience.
    fn exec_unary_op(
        &mut self,
        result: ExecRegister,
        input: ExecRegister,
        op: fn(UntypedValue) -> UntypedValue,
    ) {
        let input = self.get_register(input);
        self.set_register(result, op(input));
        self.next_instr()
    }

    /// Executes the given fallible unary `wasmi` operation.
    ///
    /// # Note
    ///
    /// Loads from the given `input` register,
    /// performs the given operation `op` and stores the
    /// result back into the `result` register.
    ///
    /// # Errors
    ///
    /// Returns an error if the given operation `op` fails.
    fn exec_fallible_unary_op(
        &mut self,
        result: ExecRegister,
        input: ExecRegister,
        op: fn(UntypedValue) -> Result<UntypedValue, TrapCode>,
    ) -> Result<(), Trap> {
        let input = self.get_register(input);
        self.set_register(result, op(input)?);
        self.next_instr();
        Ok(())
    }

    /// Loads the value of the given `provider`.
    ///
    /// # Panics
    ///
    /// If the provider refers to an non-existing immediate value.
    /// Note that reaching this case reflects a bug in the interpreter.
    fn load_provider(&self, provider: ExecProvider) -> UntypedValue {
        provider.decode_using(|rhs| self.get_register(rhs), |imm| self.resolve_cref(imm))
    }

    /// Executes the given binary `wasmi` operation.
    ///
    /// # Note
    ///
    /// Loads from the given `lhs` and `rhs` registers,
    /// performs the given operation `op` and stores the
    /// result back into the `result` register.
    ///
    /// # Errors
    ///
    /// Returns `Result::Ok` for convenience.
    fn exec_binary_reg_op(
        &mut self,
        result: ExecRegister,
        lhs: ExecRegister,
        rhs: ExecRegister,
        op: fn(UntypedValue, UntypedValue) -> UntypedValue,
    ) {
        let lhs = self.get_register(lhs);
        let rhs = self.get_register(rhs);
        self.set_register(result, op(lhs, rhs));
        self.next_instr()
    }

    /// Executes the given binary `wasmi` operation.
    ///
    /// # Note
    ///
    /// Loads from the given `lhs` and `rhs` registers,
    /// performs the given operation `op` and stores the
    /// result back into the `result` register.
    ///
    /// # Errors
    ///
    /// Returns `Result::Ok` for convenience.
    fn exec_binary_imm_op(
        &mut self,
        result: ExecRegister,
        lhs: ExecRegister,
        rhs: UntypedValue,
        op: fn(UntypedValue, UntypedValue) -> UntypedValue,
    ) {
        let lhs = self.get_register(lhs);
        self.set_register(result, op(lhs, rhs));
        self.next_instr()
    }

    /// Executes the given fallible binary `wasmi` operation.
    ///
    /// # Note
    ///
    /// Loads from the given `lhs` and `rhs` registers,
    /// performs the given operation `op` and stores the
    /// result back into the `result` register.
    ///
    /// # Errors
    ///
    /// Returns an error if the given operation `op` fails.
    fn exec_fallible_binary_reg_op(
        &mut self,
        result: ExecRegister,
        lhs: ExecRegister,
        rhs: ExecRegister,
        op: fn(UntypedValue, UntypedValue) -> Result<UntypedValue, TrapCode>,
    ) -> Result<(), Trap> {
        let lhs = self.get_register(lhs);
        let rhs = self.get_register(rhs);
        self.set_register(result, op(lhs, rhs)?);
        self.next_instr();
        Ok(())
    }

    /// Executes the given fallible binary `wasmi` operation.
    ///
    /// # Note
    ///
    /// Loads from the given `lhs` and `rhs` registers,
    /// performs the given operation `op` and stores the
    /// result back into the `result` register.
    ///
    /// # Errors
    ///
    /// Returns an error if the given operation `op` fails.
    fn exec_fallible_binary_imm_op(
        &mut self,
        result: ExecRegister,
        lhs: ExecRegister,
        rhs: UntypedValue,
        op: fn(UntypedValue, UntypedValue) -> Result<UntypedValue, TrapCode>,
    ) -> Result<(), Trap> {
        let lhs = self.get_register(lhs);
        self.set_register(result, op(lhs, rhs)?);
        self.next_instr();
        Ok(())
    }

    /// Executes a conditional branch.
    ///
    /// Only branches when `op(condition)` evaluates to `true`.
    fn exec_branch_conditionally(
        &mut self,
        target: Target,
        condition: ExecRegister,
        op: fn(UntypedValue) -> bool,
    ) {
        let condition = self.get_register(condition);
        if op(condition) {
            return self.branch_to_target(target);
        }
        self.next_instr()
    }

    /// Executes a conditional branch and copy a single value.
    ///
    /// Only branches when `op(condition)` evaluates to `true`.
    fn exec_branch_conditionally_single<F>(
        &mut self,
        target: Target,
        condition: ExecRegister,
        result: ExecRegister,
        returned: F,
        op: fn(UntypedValue) -> bool,
    ) where
        F: FnOnce(&Self) -> UntypedValue,
    {
        let condition = self.get_register(condition);
        if op(condition) {
            let returned = returned(self);
            self.set_register(result, returned);
            return self.branch_to_target(target);
        }
        self.next_instr()
    }

    /// Executes a conditional branch and copy multiple values.
    ///
    /// Only branches when `op(condition)` evaluates to `true`.
    fn exec_branch_conditionally_multi(
        &mut self,
        target: Target,
        condition: ExecRegister,
        results: ExecRegisterSlice,
        returned: ExecProviderSlice,
        op: fn(UntypedValue) -> bool,
    ) {
        let condition = self.get_register(condition);
        if op(condition) {
            self.copy_many(results, returned);
            return self.branch_to_target(target);
        }
        self.next_instr()
    }
}

impl<'engine, 'func2, 'ctx, 'cache, T> Executor<'engine, 'func2, 'ctx, 'cache, T> {
    fn exec_br(&mut self, target: Target) {
        self.branch_to_target(target)
    }

    fn exec_br_copy(
        &mut self,
        target: Target,
        result: <ExecuteTypes as InstructionTypes>::Register,
        returned: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        let returned = self.get_register(returned);
        self.set_register(result, returned);
        self.branch_to_target(target)
    }

    fn exec_br_copy_imm(
        &mut self,
        target: Target,
        result: <ExecuteTypes as InstructionTypes>::Register,
        returned: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.set_register(result, returned);
        self.branch_to_target(target)
    }

    fn exec_br_copy_multi(
        &mut self,
        target: Target,
        results: <ExecuteTypes as InstructionTypes>::RegisterSlice,
        returned: <ExecuteTypes as InstructionTypes>::ProviderSlice,
    ) {
        self.copy_many(results, returned);
        self.branch_to_target(target)
    }

    fn exec_br_eqz(
        &mut self,
        target: Target,
        condition: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_branch_conditionally(target, condition, |condition| {
            condition == UntypedValue::from(0_i32)
        })
    }

    fn exec_br_nez(
        &mut self,
        target: Target,
        condition: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_branch_conditionally(target, condition, |condition| {
            condition != UntypedValue::from(0_i32)
        })
    }

    fn exec_br_nez_copy(
        &mut self,
        target: Target,
        condition: <ExecuteTypes as InstructionTypes>::Register,
        result: <ExecuteTypes as InstructionTypes>::Register,
        returned: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_branch_conditionally_single(
            target,
            condition,
            result,
            |this| this.get_register(returned),
            |condition| condition != UntypedValue::from(0_i32),
        )
    }

    fn exec_br_nez_copy_imm(
        &mut self,
        target: Target,
        condition: <ExecuteTypes as InstructionTypes>::Register,
        result: <ExecuteTypes as InstructionTypes>::Register,
        returned: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_branch_conditionally_single(
            target,
            condition,
            result,
            |_| returned,
            |condition| condition != UntypedValue::from(0_i32),
        )
    }

    fn exec_br_nez_copy_multi(
        &mut self,
        target: Target,
        condition: <ExecuteTypes as InstructionTypes>::Register,
        results: <ExecuteTypes as InstructionTypes>::RegisterSlice,
        returned: <ExecuteTypes as InstructionTypes>::ProviderSlice,
    ) {
        self.exec_branch_conditionally_multi(target, condition, results, returned, |condition| {
            condition != UntypedValue::from(0_i32)
        })
    }

    fn exec_return_nez_impl<F>(
        &mut self,
        condition: <ExecuteTypes as InstructionTypes>::Register,
        exec_branch: F,
    ) -> ConditionalReturn
    where
        F: FnOnce(&mut Self) -> ConditionalReturn,
    {
        let condition = self.get_register(condition);
        let zero = UntypedValue::from(0_i32);
        self.pc += 1;
        if condition != zero {
            return exec_branch(self);
        }
        ConditionalReturn::Continue
    }

    fn exec_return_nez(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        condition: <ExecuteTypes as InstructionTypes>::Register,
    ) -> ConditionalReturn {
        self.exec_return_nez_impl(condition, |this| {
            let result = this.get_register(result);
            ConditionalReturn::Return { result }
        })
    }

    fn exec_return_nez_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Immediate,
        condition: <ExecuteTypes as InstructionTypes>::Register,
    ) -> ConditionalReturn {
        self.exec_return_nez_impl(condition, |_| ConditionalReturn::Return { result })
    }

    fn exec_return_nez_multi(
        &mut self,
        results: <ExecuteTypes as InstructionTypes>::ProviderSlice,
        condition: <ExecuteTypes as InstructionTypes>::Register,
    ) -> ConditionalReturnMulti {
        let condition = self.get_register(condition);
        let zero = UntypedValue::from(0_i32);
        self.pc += 1;
        if condition != zero {
            return ConditionalReturnMulti::Return { results };
        }
        ConditionalReturnMulti::Continue
    }

    fn exec_br_table(
        &mut self,
        case: <ExecuteTypes as InstructionTypes>::Register,
        len_targets: usize,
    ) {
        let index = u32::from(self.get_register(case)) as usize;
        // The index of the default target is the last target of the `br_table`.
        let max_index = len_targets - 1;
        // A normalized index will always yield a target without panicking.
        let normalized_index = cmp::min(index, max_index);
        // Simply branch to the selected instruction which is going to be either
        // a `br` or a `return` instruction as demanded by the `wasmi` bytecode.
        self.pc += normalized_index + 1;
    }

    fn exec_trap(&mut self, trap_code: TrapCode) -> Result<(), TrapCode> {
        Err(trap_code)
    }

    fn exec_return(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
    ) -> Result<CallOutcome, Trap> {
        let result = self.get_register(result);
        Ok(CallOutcome::ReturnSingle { returned: result })
    }

    fn exec_return_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Immediate,
    ) -> Result<CallOutcome, Trap> {
        Ok(CallOutcome::ReturnSingle { returned: result })
    }

    fn exec_return_multi(
        &mut self,
        results: <ExecuteTypes as InstructionTypes>::ProviderSlice,
    ) -> Result<CallOutcome, Trap> {
        Ok(CallOutcome::ReturnMulti { returned: results })
    }

    fn exec_call(
        &mut self,
        func: FuncIdx,
        results: <ExecuteTypes as InstructionTypes>::RegisterSlice,
        params: <ExecuteTypes as InstructionTypes>::ProviderSlice,
    ) -> Result<CallOutcome, Trap> {
        let callee = self.cache.get_func(&mut self.ctx, func.into_u32());
        self.call_func(callee, results, params)
    }

    fn exec_call_indirect(
        &mut self,
        func_type: FuncTypeIdx,
        results: <ExecuteTypes as InstructionTypes>::RegisterSlice,
        index: <ExecuteTypes as InstructionTypes>::Provider,
        params: <ExecuteTypes as InstructionTypes>::ProviderSlice,
    ) -> Result<CallOutcome, Trap> {
        let index = u32::from(self.load_provider(index));
        let table = self.default_table();
        let callee = table
            .get(&self.ctx, index as usize)
            .map_err(|_| TrapCode::TableAccessOutOfBounds)?
            .ok_or(TrapCode::ElemUninitialized)?;
        let actual_signature = callee.signature(&self.ctx);
        let expected_signature = self
            .frame
            .instance()
            .get_signature(&self.ctx, func_type.into_u32())
            .unwrap_or_else(|| {
                panic!(
                    "missing signature for `call_indirect` at index {:?} for instance {:?}",
                    func_type,
                    self.frame.instance()
                )
            });
        if actual_signature != expected_signature {
            return Err(Trap::from(TrapCode::UnexpectedSignature));
        }
        self.call_func(callee, results, params)
    }

    fn exec_copy(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        input: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        let input = self.get_register(input);
        self.set_register(result, input);
        self.next_instr()
    }

    fn exec_copy_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        input: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.set_register(result, input);
        self.next_instr()
    }

    fn exec_copy_many(
        &mut self,
        results: <ExecuteTypes as InstructionTypes>::RegisterSlice,
        inputs: <ExecuteTypes as InstructionTypes>::ProviderSlice,
    ) {
        self.copy_many(results, inputs);
        self.next_instr()
    }

    fn exec_select(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        condition: <ExecuteTypes as InstructionTypes>::Register,
        if_true: <ExecuteTypes as InstructionTypes>::Provider,
        if_false: <ExecuteTypes as InstructionTypes>::Provider,
    ) {
        let condition = self.get_register(condition);
        let zero = UntypedValue::from(0_i32);
        let case = if condition != zero {
            self.load_provider(if_true)
        } else {
            self.load_provider(if_false)
        };
        self.set_register(result, case);
        self.next_instr()
    }

    fn exec_global_get(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        global: bytecode::Global,
    ) {
        let value = *self.resolve_global(global);
        self.set_register(result, value);
        self.next_instr()
    }

    fn exec_global_set(
        &mut self,
        global: bytecode::Global,
        value: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        let value = self.get_register(value);
        *self.resolve_global(global) = value;
        self.next_instr()
    }

    fn exec_global_set_imm(
        &mut self,
        global: bytecode::Global,
        value: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        *self.resolve_global(global) = value;
        self.next_instr()
    }

    fn exec_i32_load(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        ptr: <ExecuteTypes as InstructionTypes>::Register,
        offset: bytecode::Offset,
    ) -> Result<(), Trap> {
        self.exec_load::<i32>(result, ptr, offset)
    }

    fn exec_i64_load(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        ptr: <ExecuteTypes as InstructionTypes>::Register,
        offset: bytecode::Offset,
    ) -> Result<(), Trap> {
        self.exec_load::<i64>(result, ptr, offset)
    }

    fn exec_f32_load(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        ptr: <ExecuteTypes as InstructionTypes>::Register,
        offset: bytecode::Offset,
    ) -> Result<(), Trap> {
        self.exec_load::<F32>(result, ptr, offset)
    }

    fn exec_f64_load(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        ptr: <ExecuteTypes as InstructionTypes>::Register,
        offset: bytecode::Offset,
    ) -> Result<(), Trap> {
        self.exec_load::<F64>(result, ptr, offset)
    }

    fn exec_i32_load_8_s(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        ptr: <ExecuteTypes as InstructionTypes>::Register,
        offset: bytecode::Offset,
    ) -> Result<(), Trap> {
        self.exec_load_extend::<i8, i32>(result, ptr, offset)
    }

    fn exec_i32_load_8_u(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        ptr: <ExecuteTypes as InstructionTypes>::Register,
        offset: bytecode::Offset,
    ) -> Result<(), Trap> {
        self.exec_load_extend::<u8, i32>(result, ptr, offset)
    }

    fn exec_i32_load_16_s(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        ptr: <ExecuteTypes as InstructionTypes>::Register,
        offset: bytecode::Offset,
    ) -> Result<(), Trap> {
        self.exec_load_extend::<i16, i32>(result, ptr, offset)
    }

    fn exec_i32_load_16_u(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        ptr: <ExecuteTypes as InstructionTypes>::Register,
        offset: bytecode::Offset,
    ) -> Result<(), Trap> {
        self.exec_load_extend::<u16, i32>(result, ptr, offset)
    }

    fn exec_i64_load_8_s(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        ptr: <ExecuteTypes as InstructionTypes>::Register,
        offset: bytecode::Offset,
    ) -> Result<(), Trap> {
        self.exec_load_extend::<i8, i64>(result, ptr, offset)
    }

    fn exec_i64_load_8_u(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        ptr: <ExecuteTypes as InstructionTypes>::Register,
        offset: bytecode::Offset,
    ) -> Result<(), Trap> {
        self.exec_load_extend::<u8, i64>(result, ptr, offset)
    }

    fn exec_i64_load_16_s(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        ptr: <ExecuteTypes as InstructionTypes>::Register,
        offset: bytecode::Offset,
    ) -> Result<(), Trap> {
        self.exec_load_extend::<i16, i64>(result, ptr, offset)
    }

    fn exec_i64_load_16_u(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        ptr: <ExecuteTypes as InstructionTypes>::Register,
        offset: bytecode::Offset,
    ) -> Result<(), Trap> {
        self.exec_load_extend::<u16, i64>(result, ptr, offset)
    }

    fn exec_i64_load_32_s(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        ptr: <ExecuteTypes as InstructionTypes>::Register,
        offset: bytecode::Offset,
    ) -> Result<(), Trap> {
        self.exec_load_extend::<i32, i64>(result, ptr, offset)
    }

    fn exec_i64_load_32_u(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        ptr: <ExecuteTypes as InstructionTypes>::Register,
        offset: bytecode::Offset,
    ) -> Result<(), Trap> {
        self.exec_load_extend::<u32, i64>(result, ptr, offset)
    }

    fn exec_i32_store(
        &mut self,
        ptr: <ExecuteTypes as InstructionTypes>::Register,
        offset: bytecode::Offset,
        value: <ExecuteTypes as InstructionTypes>::Register,
    ) -> Result<(), Trap> {
        self.exec_store::<i32>(ptr, offset, value)
    }

    fn exec_i32_store_imm(
        &mut self,
        ptr: <ExecuteTypes as InstructionTypes>::Register,
        offset: bytecode::Offset,
        value: <ExecuteTypes as InstructionTypes>::Immediate,
    ) -> Result<(), Trap> {
        self.exec_store_imm::<i32>(ptr, offset, value)
    }

    fn exec_i64_store(
        &mut self,
        ptr: <ExecuteTypes as InstructionTypes>::Register,
        offset: bytecode::Offset,
        value: <ExecuteTypes as InstructionTypes>::Register,
    ) -> Result<(), Trap> {
        self.exec_store::<i64>(ptr, offset, value)
    }

    fn exec_i64_store_imm(
        &mut self,
        ptr: <ExecuteTypes as InstructionTypes>::Register,
        offset: bytecode::Offset,
        value: <ExecuteTypes as InstructionTypes>::Immediate,
    ) -> Result<(), Trap> {
        self.exec_store_imm::<i64>(ptr, offset, value)
    }

    fn exec_f32_store(
        &mut self,
        ptr: <ExecuteTypes as InstructionTypes>::Register,
        offset: bytecode::Offset,
        value: <ExecuteTypes as InstructionTypes>::Register,
    ) -> Result<(), Trap> {
        self.exec_store::<F32>(ptr, offset, value)
    }

    fn exec_f32_store_imm(
        &mut self,
        ptr: <ExecuteTypes as InstructionTypes>::Register,
        offset: bytecode::Offset,
        value: <ExecuteTypes as InstructionTypes>::Immediate,
    ) -> Result<(), Trap> {
        self.exec_store_imm::<F32>(ptr, offset, value)
    }

    fn exec_f64_store(
        &mut self,
        ptr: <ExecuteTypes as InstructionTypes>::Register,
        offset: bytecode::Offset,
        value: <ExecuteTypes as InstructionTypes>::Register,
    ) -> Result<(), Trap> {
        self.exec_store::<F64>(ptr, offset, value)
    }

    fn exec_f64_store_imm(
        &mut self,
        ptr: <ExecuteTypes as InstructionTypes>::Register,
        offset: bytecode::Offset,
        value: <ExecuteTypes as InstructionTypes>::Immediate,
    ) -> Result<(), Trap> {
        self.exec_store_imm::<F64>(ptr, offset, value)
    }

    fn exec_i32_store_8(
        &mut self,
        ptr: <ExecuteTypes as InstructionTypes>::Register,
        offset: bytecode::Offset,
        value: <ExecuteTypes as InstructionTypes>::Register,
    ) -> Result<(), Trap> {
        self.exec_store_wrap::<i32, i8>(ptr, offset, value)
    }

    fn exec_i32_store_8_imm(
        &mut self,
        ptr: <ExecuteTypes as InstructionTypes>::Register,
        offset: bytecode::Offset,
        value: <ExecuteTypes as InstructionTypes>::Immediate,
    ) -> Result<(), Trap> {
        self.exec_store_wrap_imm::<i32, i8>(ptr, offset, value)
    }

    fn exec_i32_store_16(
        &mut self,
        ptr: <ExecuteTypes as InstructionTypes>::Register,
        offset: bytecode::Offset,
        value: <ExecuteTypes as InstructionTypes>::Register,
    ) -> Result<(), Trap> {
        self.exec_store_wrap::<i32, i16>(ptr, offset, value)
    }

    fn exec_i32_store_16_imm(
        &mut self,
        ptr: <ExecuteTypes as InstructionTypes>::Register,
        offset: bytecode::Offset,
        value: <ExecuteTypes as InstructionTypes>::Immediate,
    ) -> Result<(), Trap> {
        self.exec_store_wrap_imm::<i32, i16>(ptr, offset, value)
    }

    fn exec_i64_store_8(
        &mut self,
        ptr: <ExecuteTypes as InstructionTypes>::Register,
        offset: bytecode::Offset,
        value: <ExecuteTypes as InstructionTypes>::Register,
    ) -> Result<(), Trap> {
        self.exec_store_wrap::<i64, i8>(ptr, offset, value)
    }

    fn exec_i64_store_8_imm(
        &mut self,
        ptr: <ExecuteTypes as InstructionTypes>::Register,
        offset: bytecode::Offset,
        value: <ExecuteTypes as InstructionTypes>::Immediate,
    ) -> Result<(), Trap> {
        self.exec_store_wrap_imm::<i64, i8>(ptr, offset, value)
    }

    fn exec_i64_store_16(
        &mut self,
        ptr: <ExecuteTypes as InstructionTypes>::Register,
        offset: bytecode::Offset,
        value: <ExecuteTypes as InstructionTypes>::Register,
    ) -> Result<(), Trap> {
        self.exec_store_wrap::<i64, i16>(ptr, offset, value)
    }

    fn exec_i64_store_16_imm(
        &mut self,
        ptr: <ExecuteTypes as InstructionTypes>::Register,
        offset: bytecode::Offset,
        value: <ExecuteTypes as InstructionTypes>::Immediate,
    ) -> Result<(), Trap> {
        self.exec_store_wrap_imm::<i64, i16>(ptr, offset, value)
    }

    fn exec_i64_store_32(
        &mut self,
        ptr: <ExecuteTypes as InstructionTypes>::Register,
        offset: bytecode::Offset,
        value: <ExecuteTypes as InstructionTypes>::Register,
    ) -> Result<(), Trap> {
        self.exec_store_wrap::<i64, i32>(ptr, offset, value)
    }

    fn exec_i64_store_32_imm(
        &mut self,
        ptr: <ExecuteTypes as InstructionTypes>::Register,
        offset: bytecode::Offset,
        value: <ExecuteTypes as InstructionTypes>::Immediate,
    ) -> Result<(), Trap> {
        self.exec_store_wrap_imm::<i64, i32>(ptr, offset, value)
    }

    fn exec_memory_size(&mut self, result: <ExecuteTypes as InstructionTypes>::Register) {
        let memory = self.default_memory();
        let size = memory.current_pages(&self.ctx).0 as u32;
        self.set_register(result, size.into());
        self.next_instr()
    }

    fn exec_memory_grow(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        amount: <ExecuteTypes as InstructionTypes>::Provider,
    ) {
        let amount = u32::from(self.load_provider(amount));
        let memory = self.default_memory();
        let old_size = match memory.grow(self.ctx.as_context_mut(), Pages(amount as usize)) {
            Ok(Pages(old_size)) => old_size as u32,
            Err(_) => {
                // Note: The WebAssembly specification demands to return
                //       `0xFFFF_FFFF` for the failure case of this instruction.
                u32::MAX
            }
        };
        // The memory grow might have invalidated the cached linear memory
        // so we need to reset it in order for the cache to reload in case it
        // is used again.
        self.cache.reset_default_memory_bytes();
        self.set_register(result, old_size.into());
        self.next_instr()
    }

    fn exec_i32_eq(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::i32_eq)
    }

    fn exec_i32_eq_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::i32_eq)
    }

    fn exec_i32_ne(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::i32_ne)
    }

    fn exec_i32_ne_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::i32_ne)
    }

    fn exec_i32_lt_s(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::i32_lt_s)
    }

    fn exec_i32_lt_s_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::i32_lt_s)
    }

    fn exec_i32_lt_u(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::i32_lt_u)
    }

    fn exec_i32_lt_u_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::i32_lt_u)
    }

    fn exec_i32_gt_s(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::i32_gt_s)
    }

    fn exec_i32_gt_s_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::i32_gt_s)
    }

    fn exec_i32_gt_u(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::i32_gt_u)
    }

    fn exec_i32_gt_u_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::i32_gt_u)
    }

    fn exec_i32_le_s(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::i32_le_s)
    }

    fn exec_i32_le_s_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::i32_le_s)
    }

    fn exec_i32_le_u(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::i32_le_u)
    }

    fn exec_i32_le_u_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::i32_le_u)
    }

    fn exec_i32_ge_s(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::i32_ge_s)
    }

    fn exec_i32_ge_s_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::i32_ge_s)
    }

    fn exec_i32_ge_u(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::i32_ge_u)
    }

    fn exec_i32_ge_u_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::i32_ge_u)
    }

    fn exec_i64_eq(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::i64_eq)
    }

    fn exec_i64_eq_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::i64_eq)
    }

    fn exec_i64_ne(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::i64_ne)
    }

    fn exec_i64_ne_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::i64_ne)
    }

    fn exec_i64_lt_s(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::i64_lt_s)
    }

    fn exec_i64_lt_s_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::i64_lt_s)
    }

    fn exec_i64_lt_u(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::i64_lt_u)
    }

    fn exec_i64_lt_u_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::i64_lt_u)
    }

    fn exec_i64_gt_s(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::i64_gt_s)
    }

    fn exec_i64_gt_s_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::i64_gt_s)
    }

    fn exec_i64_gt_u(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::i64_gt_u)
    }

    fn exec_i64_gt_u_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::i64_gt_u)
    }

    fn exec_i64_le_s(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::i64_le_s)
    }

    fn exec_i64_le_s_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::i64_le_s)
    }

    fn exec_i64_le_u(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::i64_le_u)
    }

    fn exec_i64_le_u_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::i64_le_u)
    }

    fn exec_i64_ge_s(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::i64_ge_s)
    }

    fn exec_i64_ge_s_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::i64_ge_s)
    }

    fn exec_i64_ge_u(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::i64_ge_u)
    }

    fn exec_i64_ge_u_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::i64_ge_u)
    }

    fn exec_f32_eq(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::f32_eq)
    }

    fn exec_f32_eq_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::f32_eq)
    }

    fn exec_f32_ne(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::f32_ne)
    }

    fn exec_f32_ne_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::f32_ne)
    }

    fn exec_f32_lt(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::f32_lt)
    }

    fn exec_f32_lt_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::f32_lt)
    }

    fn exec_f32_gt(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::f32_gt)
    }

    fn exec_f32_gt_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::f32_gt)
    }

    fn exec_f32_le(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::f32_le)
    }

    fn exec_f32_le_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::f32_le)
    }

    fn exec_f32_ge(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::f32_ge)
    }

    fn exec_f32_ge_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::f32_ge)
    }

    fn exec_f64_eq(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::f64_eq)
    }

    fn exec_f64_eq_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::f64_eq)
    }

    fn exec_f64_ne(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::f64_ne)
    }

    fn exec_f64_ne_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::f64_ne)
    }

    fn exec_f64_lt(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::f64_lt)
    }

    fn exec_f64_lt_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::f64_lt)
    }

    fn exec_f64_gt(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::f64_gt)
    }

    fn exec_f64_gt_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::f64_gt)
    }

    fn exec_f64_le(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::f64_le)
    }

    fn exec_f64_le_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::f64_le)
    }

    fn exec_f64_ge(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::f64_ge)
    }

    fn exec_f64_ge_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::f64_ge)
    }

    fn exec_i32_clz(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        input: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_unary_op(result, input, UntypedValue::i32_clz)
    }

    fn exec_i32_ctz(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        input: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_unary_op(result, input, UntypedValue::i32_ctz)
    }

    fn exec_i32_popcnt(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        input: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_unary_op(result, input, UntypedValue::i32_popcnt)
    }

    fn exec_i32_add(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::i32_add)
    }

    fn exec_i32_add_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::i32_add)
    }

    fn exec_i32_sub(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::i32_sub)
    }

    fn exec_i32_sub_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::i32_sub)
    }

    fn exec_i32_mul(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::i32_mul)
    }

    fn exec_i32_mul_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::i32_mul)
    }

    fn exec_i32_div_s(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) -> Result<(), Trap> {
        self.exec_fallible_binary_reg_op(result, lhs, rhs, UntypedValue::i32_div_s)
    }

    fn exec_i32_div_s_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) -> Result<(), Trap> {
        self.exec_fallible_binary_imm_op(result, lhs, rhs, UntypedValue::i32_div_s)
    }

    fn exec_i32_div_u(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) -> Result<(), Trap> {
        self.exec_fallible_binary_reg_op(result, lhs, rhs, UntypedValue::i32_div_u)
    }

    fn exec_i32_div_u_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) -> Result<(), Trap> {
        self.exec_fallible_binary_imm_op(result, lhs, rhs, UntypedValue::i32_div_u)
    }

    fn exec_i32_rem_s(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) -> Result<(), Trap> {
        self.exec_fallible_binary_reg_op(result, lhs, rhs, UntypedValue::i32_rem_s)
    }

    fn exec_i32_rem_s_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) -> Result<(), Trap> {
        self.exec_fallible_binary_imm_op(result, lhs, rhs, UntypedValue::i32_rem_s)
    }

    fn exec_i32_rem_u(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) -> Result<(), Trap> {
        self.exec_fallible_binary_reg_op(result, lhs, rhs, UntypedValue::i32_rem_u)
    }

    fn exec_i32_rem_u_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) -> Result<(), Trap> {
        self.exec_fallible_binary_imm_op(result, lhs, rhs, UntypedValue::i32_rem_u)
    }

    fn exec_i32_and(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::i32_and)
    }

    fn exec_i32_and_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::i32_and)
    }

    fn exec_i32_or(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::i32_or)
    }

    fn exec_i32_or_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::i32_or)
    }

    fn exec_i32_xor(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::i32_xor)
    }

    fn exec_i32_xor_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::i32_xor)
    }

    fn exec_i32_shl(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::i32_shl)
    }

    fn exec_i32_shl_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::i32_shl)
    }

    fn exec_i32_shr_s(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::i32_shr_s)
    }

    fn exec_i32_shr_s_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::i32_shr_s)
    }

    fn exec_i32_shr_u(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::i32_shr_u)
    }

    fn exec_i32_shr_u_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::i32_shr_u)
    }

    fn exec_i32_rotl(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::i32_rotl)
    }

    fn exec_i32_rotl_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::i32_rotl)
    }

    fn exec_i32_rotr(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::i32_rotr)
    }

    fn exec_i32_rotr_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::i32_rotr)
    }

    fn exec_i64_clz(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        input: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_unary_op(result, input, UntypedValue::i64_clz)
    }

    fn exec_i64_ctz(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        input: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_unary_op(result, input, UntypedValue::i64_ctz)
    }

    fn exec_i64_popcnt(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        input: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_unary_op(result, input, UntypedValue::i64_popcnt)
    }

    fn exec_i64_add(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::i64_add)
    }

    fn exec_i64_add_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::i64_add)
    }

    fn exec_i64_sub(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::i64_sub)
    }

    fn exec_i64_sub_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::i64_sub)
    }

    fn exec_i64_mul(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::i64_mul)
    }

    fn exec_i64_mul_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::i64_mul)
    }

    fn exec_i64_div_s(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) -> Result<(), Trap> {
        self.exec_fallible_binary_reg_op(result, lhs, rhs, UntypedValue::i64_div_s)
    }

    fn exec_i64_div_s_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) -> Result<(), Trap> {
        self.exec_fallible_binary_imm_op(result, lhs, rhs, UntypedValue::i64_div_s)
    }

    fn exec_i64_div_u(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) -> Result<(), Trap> {
        self.exec_fallible_binary_reg_op(result, lhs, rhs, UntypedValue::i64_div_u)
    }

    fn exec_i64_div_u_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) -> Result<(), Trap> {
        self.exec_fallible_binary_imm_op(result, lhs, rhs, UntypedValue::i64_div_u)
    }

    fn exec_i64_rem_s(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) -> Result<(), Trap> {
        self.exec_fallible_binary_reg_op(result, lhs, rhs, UntypedValue::i64_rem_s)
    }

    fn exec_i64_rem_s_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) -> Result<(), Trap> {
        self.exec_fallible_binary_imm_op(result, lhs, rhs, UntypedValue::i64_rem_s)
    }

    fn exec_i64_rem_u(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) -> Result<(), Trap> {
        self.exec_fallible_binary_reg_op(result, lhs, rhs, UntypedValue::i64_rem_u)
    }

    fn exec_i64_rem_u_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) -> Result<(), Trap> {
        self.exec_fallible_binary_imm_op(result, lhs, rhs, UntypedValue::i64_rem_u)
    }

    fn exec_i64_and(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::i64_and)
    }

    fn exec_i64_and_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::i64_and)
    }

    fn exec_i64_or(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::i64_or)
    }

    fn exec_i64_or_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::i64_or)
    }

    fn exec_i64_xor(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::i64_xor)
    }

    fn exec_i64_xor_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::i64_xor)
    }

    fn exec_i64_shl(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::i64_shl)
    }

    fn exec_i64_shl_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::i64_shl)
    }

    fn exec_i64_shr_s(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::i64_shr_s)
    }

    fn exec_i64_shr_s_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::i64_shr_s)
    }

    fn exec_i64_shr_u(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::i64_shr_u)
    }

    fn exec_i64_shr_u_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::i64_shr_u)
    }

    fn exec_i64_rotl(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::i64_rotl)
    }

    fn exec_i64_rotl_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::i64_rotl)
    }

    fn exec_i64_rotr(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::i64_rotr)
    }

    fn exec_i64_rotr_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::i64_rotr)
    }

    fn exec_f32_abs(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        input: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_unary_op(result, input, UntypedValue::f32_abs)
    }

    fn exec_f32_neg(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        input: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_unary_op(result, input, UntypedValue::f32_neg)
    }

    fn exec_f32_ceil(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        input: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_unary_op(result, input, UntypedValue::f32_ceil)
    }

    fn exec_f32_floor(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        input: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_unary_op(result, input, UntypedValue::f32_floor)
    }

    fn exec_f32_trunc(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        input: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_unary_op(result, input, UntypedValue::f32_trunc)
    }

    fn exec_f32_nearest(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        input: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_unary_op(result, input, UntypedValue::f32_nearest)
    }

    fn exec_f32_sqrt(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        input: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_unary_op(result, input, UntypedValue::f32_sqrt)
    }

    fn exec_f32_add(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::f32_add)
    }

    fn exec_f32_add_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::f32_add)
    }

    fn exec_f32_sub(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::f32_sub)
    }

    fn exec_f32_sub_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::f32_sub)
    }

    fn exec_f32_mul(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::f32_mul)
    }

    fn exec_f32_mul_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::f32_mul)
    }

    fn exec_f32_div(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) -> Result<(), Trap> {
        self.exec_fallible_binary_reg_op(result, lhs, rhs, UntypedValue::f32_div)
    }

    fn exec_f32_div_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) -> Result<(), Trap> {
        self.exec_fallible_binary_imm_op(result, lhs, rhs, UntypedValue::f32_div)
    }

    fn exec_f32_min(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::f32_min)
    }

    fn exec_f32_min_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::f32_min)
    }

    fn exec_f32_max(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::f32_max)
    }

    fn exec_f32_max_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::f32_max)
    }

    fn exec_f32_copysign(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::f32_copysign)
    }

    fn exec_f32_copysign_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::f32_copysign)
    }

    fn exec_f64_abs(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        input: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_unary_op(result, input, UntypedValue::f64_abs)
    }

    fn exec_f64_neg(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        input: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_unary_op(result, input, UntypedValue::f64_neg)
    }

    fn exec_f64_ceil(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        input: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_unary_op(result, input, UntypedValue::f64_ceil)
    }

    fn exec_f64_floor(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        input: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_unary_op(result, input, UntypedValue::f64_floor)
    }

    fn exec_f64_trunc(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        input: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_unary_op(result, input, UntypedValue::f64_trunc)
    }

    fn exec_f64_nearest(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        input: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_unary_op(result, input, UntypedValue::f64_nearest)
    }

    fn exec_f64_sqrt(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        input: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_unary_op(result, input, UntypedValue::f64_sqrt)
    }

    fn exec_f64_add(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::f64_add)
    }

    fn exec_f64_add_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::f64_add)
    }

    fn exec_f64_sub(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::f64_sub)
    }

    fn exec_f64_sub_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::f64_sub)
    }

    fn exec_f64_mul(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::f64_mul)
    }

    fn exec_f64_mul_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::f64_mul)
    }

    fn exec_f64_div(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) -> Result<(), Trap> {
        self.exec_fallible_binary_reg_op(result, lhs, rhs, UntypedValue::f64_div)
    }

    fn exec_f64_div_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) -> Result<(), Trap> {
        self.exec_fallible_binary_imm_op(result, lhs, rhs, UntypedValue::f64_div)
    }

    fn exec_f64_min(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::f64_min)
    }

    fn exec_f64_min_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::f64_min)
    }

    fn exec_f64_max(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::f64_max)
    }

    fn exec_f64_max_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::f64_max)
    }

    fn exec_f64_copysign(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_binary_reg_op(result, lhs, rhs, UntypedValue::f64_copysign)
    }

    fn exec_f64_copysign_imm(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        lhs: <ExecuteTypes as InstructionTypes>::Register,
        rhs: <ExecuteTypes as InstructionTypes>::Immediate,
    ) {
        self.exec_binary_imm_op(result, lhs, rhs, UntypedValue::f64_copysign)
    }

    fn exec_i32_wrap_i64(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        input: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_unary_op(result, input, UntypedValue::i32_wrap_i64)
    }

    fn exec_i32_trunc_f32_s(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        input: <ExecuteTypes as InstructionTypes>::Register,
    ) -> Result<(), Trap> {
        self.exec_fallible_unary_op(result, input, UntypedValue::i32_trunc_f32_s)
    }

    fn exec_i32_trunc_f32_u(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        input: <ExecuteTypes as InstructionTypes>::Register,
    ) -> Result<(), Trap> {
        self.exec_fallible_unary_op(result, input, UntypedValue::i32_trunc_f32_u)
    }

    fn exec_i32_trunc_f64_s(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        input: <ExecuteTypes as InstructionTypes>::Register,
    ) -> Result<(), Trap> {
        self.exec_fallible_unary_op(result, input, UntypedValue::i32_trunc_f64_s)
    }

    fn exec_i32_trunc_f64_u(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        input: <ExecuteTypes as InstructionTypes>::Register,
    ) -> Result<(), Trap> {
        self.exec_fallible_unary_op(result, input, UntypedValue::i32_trunc_f64_u)
    }

    fn exec_i64_extend_i32_s(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        input: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_unary_op(result, input, UntypedValue::i64_extend_i32_s)
    }

    fn exec_i64_extend_i32_u(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        input: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_unary_op(result, input, UntypedValue::i64_extend_i32_u)
    }

    fn exec_i64_trunc_f32_s(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        input: <ExecuteTypes as InstructionTypes>::Register,
    ) -> Result<(), Trap> {
        self.exec_fallible_unary_op(result, input, UntypedValue::i64_trunc_f32_s)
    }

    fn exec_i64_trunc_f32_u(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        input: <ExecuteTypes as InstructionTypes>::Register,
    ) -> Result<(), Trap> {
        self.exec_fallible_unary_op(result, input, UntypedValue::i64_trunc_f32_u)
    }

    fn exec_i64_trunc_f64_s(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        input: <ExecuteTypes as InstructionTypes>::Register,
    ) -> Result<(), Trap> {
        self.exec_fallible_unary_op(result, input, UntypedValue::i64_trunc_f64_s)
    }

    fn exec_i64_trunc_f64_u(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        input: <ExecuteTypes as InstructionTypes>::Register,
    ) -> Result<(), Trap> {
        self.exec_fallible_unary_op(result, input, UntypedValue::i64_trunc_f64_u)
    }

    fn exec_f32_convert_i32_s(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        input: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_unary_op(result, input, UntypedValue::f32_convert_i32_s)
    }

    fn exec_f32_convert_i32_u(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        input: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_unary_op(result, input, UntypedValue::f32_convert_i32_u)
    }

    fn exec_f32_convert_i64_s(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        input: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_unary_op(result, input, UntypedValue::f32_convert_i64_s)
    }

    fn exec_f32_convert_i64_u(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        input: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_unary_op(result, input, UntypedValue::f32_convert_i64_u)
    }

    fn exec_f32_demote_f64(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        input: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_unary_op(result, input, UntypedValue::f32_demote_f64)
    }

    fn exec_f64_convert_i32_s(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        input: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_unary_op(result, input, UntypedValue::f64_convert_i32_s)
    }

    fn exec_f64_convert_i32_u(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        input: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_unary_op(result, input, UntypedValue::f64_convert_i32_u)
    }

    fn exec_f64_convert_i64_s(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        input: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_unary_op(result, input, UntypedValue::f64_convert_i64_s)
    }

    fn exec_f64_convert_i64_u(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        input: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_unary_op(result, input, UntypedValue::f64_convert_i64_u)
    }

    fn exec_f64_promote_f32(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        input: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_unary_op(result, input, UntypedValue::f64_promote_f32)
    }

    fn exec_i32_extend8_s(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        input: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_unary_op(result, input, UntypedValue::i32_extend8_s)
    }

    fn exec_i32_extend16_s(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        input: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_unary_op(result, input, UntypedValue::i32_extend16_s)
    }

    fn exec_i64_extend8_s(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        input: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_unary_op(result, input, UntypedValue::i64_extend8_s)
    }

    fn exec_i64_extend16_s(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        input: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_unary_op(result, input, UntypedValue::i64_extend16_s)
    }

    fn exec_i64_extend32_s(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        input: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_unary_op(result, input, UntypedValue::i64_extend32_s)
    }

    fn exec_i32_trunc_sat_f32_s(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        input: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_unary_op(result, input, UntypedValue::i32_trunc_sat_f32_s)
    }

    fn exec_i32_trunc_sat_f32_u(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        input: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_unary_op(result, input, UntypedValue::i32_trunc_sat_f32_u)
    }

    fn exec_i32_trunc_sat_f64_s(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        input: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_unary_op(result, input, UntypedValue::i32_trunc_sat_f64_s)
    }

    fn exec_i32_trunc_sat_f64_u(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        input: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_unary_op(result, input, UntypedValue::i32_trunc_sat_f64_u)
    }

    fn exec_i64_trunc_sat_f32_s(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        input: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_unary_op(result, input, UntypedValue::i64_trunc_sat_f32_s)
    }

    fn exec_i64_trunc_sat_f32_u(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        input: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_unary_op(result, input, UntypedValue::i64_trunc_sat_f32_u)
    }

    fn exec_i64_trunc_sat_f64_s(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        input: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_unary_op(result, input, UntypedValue::i64_trunc_sat_f64_s)
    }

    fn exec_i64_trunc_sat_f64_u(
        &mut self,
        result: <ExecuteTypes as InstructionTypes>::Register,
        input: <ExecuteTypes as InstructionTypes>::Register,
    ) {
        self.exec_unary_op(result, input, UntypedValue::i64_trunc_sat_f64_u)
    }
}