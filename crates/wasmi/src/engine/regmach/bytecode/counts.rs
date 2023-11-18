use super::Instruction;

#[derive(Default)]
#[allow(non_snake_case)]
pub struct InstructionCounts {
    TableIdx: usize,
    DataSegmentIdx: usize,
    ElementSegmentIdx: usize,
    Const32: usize,
    I64Const32: usize,
    F64Const32: usize,
    Register: usize,
    Register2: usize,
    Register3: usize,
    RegisterList: usize,
    CallIndirectParams: usize,
    CallIndirectParamsImm16: usize,
    Trap: usize,
    ConsumeFuel: usize,
    Return: usize,
    ReturnReg: usize,
    ReturnReg2: usize,
    ReturnReg3: usize,
    ReturnImm32: usize,
    ReturnI64Imm32: usize,
    ReturnF64Imm32: usize,
    ReturnSpan: usize,
    ReturnMany: usize,
    ReturnNez: usize,
    ReturnNezReg: usize,
    ReturnNezReg2: usize,
    ReturnNezImm32: usize,
    ReturnNezI64Imm32: usize,
    ReturnNezF64Imm32: usize,
    ReturnNezSpan: usize,
    ReturnNezMany: usize,
    Branch: usize,
    BranchEqz: usize,
    BranchNez: usize,
    BranchTable: usize,
    Copy: usize,
    Copy2: usize,
    CopyImm32: usize,
    CopyI64Imm32: usize,
    CopyF64Imm32: usize,
    CopySpan: usize,
    CopySpanNonOverlapping: usize,
    CopyMany: usize,
    CopyManyNonOverlapping: usize,
    ReturnCallInternal0: usize,
    ReturnCallInternal: usize,
    ReturnCallImported0: usize,
    ReturnCallImported: usize,
    ReturnCallIndirect0: usize,
    ReturnCallIndirect: usize,
    CallInternal0: usize,
    CallInternal: usize,
    CallImported0: usize,
    CallImported: usize,
    CallIndirect0: usize,
    CallIndirect: usize,
    Select: usize,
    SelectRev: usize,
    SelectImm32: usize,
    SelectI64Imm32: usize,
    SelectF64Imm32: usize,
    RefFunc: usize,
    TableGet: usize,
    TableGetImm: usize,
    TableSize: usize,
    TableSet: usize,
    TableSetAt: usize,
    TableCopy: usize,
    TableCopyTo: usize,
    TableCopyFrom: usize,
    TableCopyFromTo: usize,
    TableCopyExact: usize,
    TableCopyToExact: usize,
    TableCopyFromExact: usize,
    TableCopyFromToExact: usize,
    TableInit: usize,
    TableInitTo: usize,
    TableInitFrom: usize,
    TableInitFromTo: usize,
    TableInitExact: usize,
    TableInitToExact: usize,
    TableInitFromExact: usize,
    TableInitFromToExact: usize,
    TableFill: usize,
    TableFillAt: usize,
    TableFillExact: usize,
    TableFillAtExact: usize,
    TableGrow: usize,
    TableGrowImm: usize,
    ElemDrop: usize,
    DataDrop: usize,
    MemorySize: usize,
    MemoryGrow: usize,
    MemoryGrowBy: usize,
    MemoryCopy: usize,
    MemoryCopyTo: usize,
    MemoryCopyFrom: usize,
    MemoryCopyFromTo: usize,
    MemoryCopyExact: usize,
    MemoryCopyToExact: usize,
    MemoryCopyFromExact: usize,
    MemoryCopyFromToExact: usize,
    MemoryFill: usize,
    MemoryFillAt: usize,
    MemoryFillImm: usize,
    MemoryFillExact: usize,
    MemoryFillAtImm: usize,
    MemoryFillAtExact: usize,
    MemoryFillImmExact: usize,
    MemoryFillAtImmExact: usize,
    MemoryInit: usize,
    MemoryInitTo: usize,
    MemoryInitFrom: usize,
    MemoryInitFromTo: usize,
    MemoryInitExact: usize,
    MemoryInitToExact: usize,
    MemoryInitFromExact: usize,
    MemoryInitFromToExact: usize,
    GlobalGet: usize,
    GlobalSet: usize,
    GlobalSetI32Imm16: usize,
    GlobalSetI64Imm16: usize,
    I32Load: usize,
    I32LoadAt: usize,
    I32LoadOffset16: usize,
    I64Load: usize,
    I64LoadAt: usize,
    I64LoadOffset16: usize,
    F32Load: usize,
    F32LoadAt: usize,
    F32LoadOffset16: usize,
    F64Load: usize,
    F64LoadAt: usize,
    F64LoadOffset16: usize,
    I32Load8s: usize,
    I32Load8sAt: usize,
    I32Load8sOffset16: usize,
    I32Load8u: usize,
    I32Load8uAt: usize,
    I32Load8uOffset16: usize,
    I32Load16s: usize,
    I32Load16sAt: usize,
    I32Load16sOffset16: usize,
    I32Load16u: usize,
    I32Load16uAt: usize,
    I32Load16uOffset16: usize,
    I64Load8s: usize,
    I64Load8sAt: usize,
    I64Load8sOffset16: usize,
    I64Load8u: usize,
    I64Load8uAt: usize,
    I64Load8uOffset16: usize,
    I64Load16s: usize,
    I64Load16sAt: usize,
    I64Load16sOffset16: usize,
    I64Load16u: usize,
    I64Load16uAt: usize,
    I64Load16uOffset16: usize,
    I64Load32s: usize,
    I64Load32sAt: usize,
    I64Load32sOffset16: usize,
    I64Load32u: usize,
    I64Load32uAt: usize,
    I64Load32uOffset16: usize,
    I32Store: usize,
    I32StoreOffset16: usize,
    I32StoreOffset16Imm16: usize,
    I32StoreAt: usize,
    I32StoreAtImm16: usize,
    I32Store8: usize,
    I32Store8Offset16: usize,
    I32Store8Offset16Imm: usize,
    I32Store8At: usize,
    I32Store8AtImm: usize,
    I32Store16: usize,
    I32Store16Offset16: usize,
    I32Store16Offset16Imm: usize,
    I32Store16At: usize,
    I32Store16AtImm: usize,
    I64Store: usize,
    I64StoreOffset16: usize,
    I64StoreOffset16Imm16: usize,
    I64StoreAt: usize,
    I64StoreAtImm16: usize,
    I64Store8: usize,
    I64Store8Offset16: usize,
    I64Store8Offset16Imm: usize,
    I64Store8At: usize,
    I64Store8AtImm: usize,
    I64Store16: usize,
    I64Store16Offset16: usize,
    I64Store16Offset16Imm: usize,
    I64Store16At: usize,
    I64Store16AtImm: usize,
    I64Store32: usize,
    I64Store32Offset16: usize,
    I64Store32Offset16Imm16: usize,
    I64Store32At: usize,
    I64Store32AtImm16: usize,
    F32Store: usize,
    F32StoreOffset16: usize,
    F32StoreAt: usize,
    F64Store: usize,
    F64StoreOffset16: usize,
    F64StoreAt: usize,
    I32Eq: usize,
    I32EqImm16: usize,
    I64Eq: usize,
    I64EqImm16: usize,
    I32Ne: usize,
    I32NeImm16: usize,
    I64Ne: usize,
    I64NeImm16: usize,
    I32LtS: usize,
    I32LtU: usize,
    I32LtSImm16: usize,
    I32LtUImm16: usize,
    I64LtS: usize,
    I64LtU: usize,
    I64LtSImm16: usize,
    I64LtUImm16: usize,
    I32GtS: usize,
    I32GtU: usize,
    I32GtSImm16: usize,
    I32GtUImm16: usize,
    I64GtS: usize,
    I64GtU: usize,
    I64GtSImm16: usize,
    I64GtUImm16: usize,
    I32LeS: usize,
    I32LeU: usize,
    I32LeSImm16: usize,
    I32LeUImm16: usize,
    I64LeS: usize,
    I64LeU: usize,
    I64LeSImm16: usize,
    I64LeUImm16: usize,
    I32GeS: usize,
    I32GeU: usize,
    I32GeSImm16: usize,
    I32GeUImm16: usize,
    I64GeS: usize,
    I64GeU: usize,
    I64GeSImm16: usize,
    I64GeUImm16: usize,
    F32Eq: usize,
    F64Eq: usize,
    F32Ne: usize,
    F64Ne: usize,
    F32Lt: usize,
    F64Lt: usize,
    F32Le: usize,
    F64Le: usize,
    F32Gt: usize,
    F64Gt: usize,
    F32Ge: usize,
    F64Ge: usize,
    I32Clz: usize,
    I64Clz: usize,
    I32Ctz: usize,
    I64Ctz: usize,
    I32Popcnt: usize,
    I64Popcnt: usize,
    I32Add: usize,
    I64Add: usize,
    I32AddImm16: usize,
    I64AddImm16: usize,
    I32Sub: usize,
    I64Sub: usize,
    I32SubImm16: usize,
    I64SubImm16: usize,
    I32SubImm16Rev: usize,
    I64SubImm16Rev: usize,
    I32Mul: usize,
    I64Mul: usize,
    I32MulImm16: usize,
    I64MulImm16: usize,
    I32DivS: usize,
    I64DivS: usize,
    I32DivSImm16: usize,
    I64DivSImm16: usize,
    I32DivSImm16Rev: usize,
    I64DivSImm16Rev: usize,
    I32DivU: usize,
    I64DivU: usize,
    I32DivUImm16: usize,
    I64DivUImm16: usize,
    I32DivUImm16Rev: usize,
    I64DivUImm16Rev: usize,
    I32RemS: usize,
    I64RemS: usize,
    I32RemSImm16: usize,
    I64RemSImm16: usize,
    I32RemSImm16Rev: usize,
    I64RemSImm16Rev: usize,
    I32RemU: usize,
    I64RemU: usize,
    I32RemUImm16: usize,
    I64RemUImm16: usize,
    I32RemUImm16Rev: usize,
    I64RemUImm16Rev: usize,
    I32And: usize,
    I64And: usize,
    I32AndImm16: usize,
    I64AndImm16: usize,
    I32Or: usize,
    I64Or: usize,
    I32OrImm16: usize,
    I64OrImm16: usize,
    I32Xor: usize,
    I64Xor: usize,
    I32XorImm16: usize,
    I64XorImm16: usize,
    I32Shl: usize,
    I64Shl: usize,
    I32ShlImm: usize,
    I64ShlImm: usize,
    I32ShlImm16Rev: usize,
    I64ShlImm16Rev: usize,
    I32ShrU: usize,
    I64ShrU: usize,
    I32ShrUImm: usize,
    I64ShrUImm: usize,
    I32ShrUImm16Rev: usize,
    I64ShrUImm16Rev: usize,
    I32ShrS: usize,
    I64ShrS: usize,
    I32ShrSImm: usize,
    I64ShrSImm: usize,
    I32ShrSImm16Rev: usize,
    I64ShrSImm16Rev: usize,
    I32Rotl: usize,
    I64Rotl: usize,
    I32RotlImm: usize,
    I64RotlImm: usize,
    I32RotlImm16Rev: usize,
    I64RotlImm16Rev: usize,
    I32Rotr: usize,
    I64Rotr: usize,
    I32RotrImm: usize,
    I64RotrImm: usize,
    I32RotrImm16Rev: usize,
    I64RotrImm16Rev: usize,
    F32Abs: usize,
    F64Abs: usize,
    F32Neg: usize,
    F64Neg: usize,
    F32Ceil: usize,
    F64Ceil: usize,
    F32Floor: usize,
    F64Floor: usize,
    F32Trunc: usize,
    F64Trunc: usize,
    F32Nearest: usize,
    F64Nearest: usize,
    F32Sqrt: usize,
    F64Sqrt: usize,
    F32Add: usize,
    F64Add: usize,
    F32Sub: usize,
    F64Sub: usize,
    F32Mul: usize,
    F64Mul: usize,
    F32Div: usize,
    F64Div: usize,
    F32Min: usize,
    F64Min: usize,
    F32Max: usize,
    F64Max: usize,
    F32Copysign: usize,
    F64Copysign: usize,
    F32CopysignImm: usize,
    F64CopysignImm: usize,
    I32WrapI64: usize,
    I64ExtendI32S: usize,
    I64ExtendI32U: usize,
    I32TruncF32S: usize,
    I32TruncF32U: usize,
    I32TruncF64S: usize,
    I32TruncF64U: usize,
    I64TruncF32S: usize,
    I64TruncF32U: usize,
    I64TruncF64S: usize,
    I64TruncF64U: usize,
    I32TruncSatF32S: usize,
    I32TruncSatF32U: usize,
    I32TruncSatF64S: usize,
    I32TruncSatF64U: usize,
    I64TruncSatF32S: usize,
    I64TruncSatF32U: usize,
    I64TruncSatF64S: usize,
    I64TruncSatF64U: usize,
    I32Extend8S: usize,
    I32Extend16S: usize,
    I64Extend8S: usize,
    I64Extend16S: usize,
    I64Extend32S: usize,
    F32DemoteF64: usize,
    F64PromoteF32: usize,
    F32ConvertI32S: usize,
    F32ConvertI32U: usize,
    F32ConvertI64S: usize,
    F32ConvertI64U: usize,
    F64ConvertI32S: usize,
    F64ConvertI32U: usize,
    F64ConvertI64S: usize,
    F64ConvertI64U: usize,
}

impl core::fmt::Debug for InstructionCounts {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut buffer = Vec::new();
        let mut push = |ident: &'static str, count: usize| {
            if count > 0 {
                buffer.push((ident, count));
            }
        };
        push("TableIdx", self.TableIdx);
        push("DataSegmentIdx", self.DataSegmentIdx);
        push("ElementSegmentIdx", self.ElementSegmentIdx);
        push("Const32", self.Const32);
        push("I64Const32", self.I64Const32);
        push("F64Const32", self.F64Const32);
        push("Register", self.Register);
        push("Register2", self.Register2);
        push("Register3", self.Register3);
        push("RegisterList", self.RegisterList);
        push("CallIndirectParams", self.CallIndirectParams);
        push("CallIndirectParamsImm16", self.CallIndirectParamsImm16);
        push("Trap", self.Trap);
        push("ConsumeFuel", self.ConsumeFuel);
        push("Return", self.Return);
        push("ReturnReg", self.ReturnReg);
        push("ReturnReg2", self.ReturnReg2);
        push("ReturnReg3", self.ReturnReg3);
        push("ReturnImm32", self.ReturnImm32);
        push("ReturnI64Imm32", self.ReturnI64Imm32);
        push("ReturnF64Imm32", self.ReturnF64Imm32);
        push("ReturnSpan", self.ReturnSpan);
        push("ReturnMany", self.ReturnMany);
        push("ReturnNez", self.ReturnNez);
        push("ReturnNezReg", self.ReturnNezReg);
        push("ReturnNezReg2", self.ReturnNezReg2);
        push("ReturnNezImm32", self.ReturnNezImm32);
        push("ReturnNezI64Imm32", self.ReturnNezI64Imm32);
        push("ReturnNezF64Imm32", self.ReturnNezF64Imm32);
        push("ReturnNezSpan", self.ReturnNezSpan);
        push("ReturnNezMany", self.ReturnNezMany);
        push("Branch", self.Branch);
        push("BranchEqz", self.BranchEqz);
        push("BranchNez", self.BranchNez);
        push("BranchTable", self.BranchTable);
        push("Copy", self.Copy);
        push("Copy2", self.Copy2);
        push("CopyImm32", self.CopyImm32);
        push("CopyI64Imm32", self.CopyI64Imm32);
        push("CopyF64Imm32", self.CopyF64Imm32);
        push("CopySpan", self.CopySpan);
        push("CopySpanNonOverlapping", self.CopySpanNonOverlapping);
        push("CopyMany", self.CopyMany);
        push("CopyManyNonOverlapping", self.CopyManyNonOverlapping);
        push("ReturnCallInternal0", self.ReturnCallInternal0);
        push("ReturnCallInternal", self.ReturnCallInternal);
        push("ReturnCallImported0", self.ReturnCallImported0);
        push("ReturnCallImported", self.ReturnCallImported);
        push("ReturnCallIndirect0", self.ReturnCallIndirect0);
        push("ReturnCallIndirect", self.ReturnCallIndirect);
        push("CallInternal0", self.CallInternal0);
        push("CallInternal", self.CallInternal);
        push("CallImported0", self.CallImported0);
        push("CallImported", self.CallImported);
        push("CallIndirect0", self.CallIndirect0);
        push("CallIndirect", self.CallIndirect);
        push("Select", self.Select);
        push("SelectRev", self.SelectRev);
        push("SelectImm32", self.SelectImm32);
        push("SelectI64Imm32", self.SelectI64Imm32);
        push("SelectF64Imm32", self.SelectF64Imm32);
        push("RefFunc", self.RefFunc);
        push("TableGet", self.TableGet);
        push("TableGetImm", self.TableGetImm);
        push("TableSize", self.TableSize);
        push("TableSet", self.TableSet);
        push("TableSetAt", self.TableSetAt);
        push("TableCopy", self.TableCopy);
        push("TableCopyTo", self.TableCopyTo);
        push("TableCopyFrom", self.TableCopyFrom);
        push("TableCopyFromTo", self.TableCopyFromTo);
        push("TableCopyExact", self.TableCopyExact);
        push("TableCopyToExact", self.TableCopyToExact);
        push("TableCopyFromExact", self.TableCopyFromExact);
        push("TableCopyFromToExact", self.TableCopyFromToExact);
        push("TableInit", self.TableInit);
        push("TableInitTo", self.TableInitTo);
        push("TableInitFrom", self.TableInitFrom);
        push("TableInitFromTo", self.TableInitFromTo);
        push("TableInitExact", self.TableInitExact);
        push("TableInitToExact", self.TableInitToExact);
        push("TableInitFromExact", self.TableInitFromExact);
        push("TableInitFromToExact", self.TableInitFromToExact);
        push("TableFill", self.TableFill);
        push("TableFillAt", self.TableFillAt);
        push("TableFillExact", self.TableFillExact);
        push("TableFillAtExact", self.TableFillAtExact);
        push("TableGrow", self.TableGrow);
        push("TableGrowImm", self.TableGrowImm);
        push("ElemDrop", self.ElemDrop);
        push("DataDrop", self.DataDrop);
        push("MemorySize", self.MemorySize);
        push("MemoryGrow", self.MemoryGrow);
        push("MemoryGrowBy", self.MemoryGrowBy);
        push("MemoryCopy", self.MemoryCopy);
        push("MemoryCopyTo", self.MemoryCopyTo);
        push("MemoryCopyFrom", self.MemoryCopyFrom);
        push("MemoryCopyFromTo", self.MemoryCopyFromTo);
        push("MemoryCopyExact", self.MemoryCopyExact);
        push("MemoryCopyToExact", self.MemoryCopyToExact);
        push("MemoryCopyFromExact", self.MemoryCopyFromExact);
        push("MemoryCopyFromToExact", self.MemoryCopyFromToExact);
        push("MemoryFill", self.MemoryFill);
        push("MemoryFillAt", self.MemoryFillAt);
        push("MemoryFillImm", self.MemoryFillImm);
        push("MemoryFillExact", self.MemoryFillExact);
        push("MemoryFillAtImm", self.MemoryFillAtImm);
        push("MemoryFillAtExact", self.MemoryFillAtExact);
        push("MemoryFillImmExact", self.MemoryFillImmExact);
        push("MemoryFillAtImmExact", self.MemoryFillAtImmExact);
        push("MemoryInit", self.MemoryInit);
        push("MemoryInitTo", self.MemoryInitTo);
        push("MemoryInitFrom", self.MemoryInitFrom);
        push("MemoryInitFromTo", self.MemoryInitFromTo);
        push("MemoryInitExact", self.MemoryInitExact);
        push("MemoryInitToExact", self.MemoryInitToExact);
        push("MemoryInitFromExact", self.MemoryInitFromExact);
        push("MemoryInitFromToExact", self.MemoryInitFromToExact);
        push("GlobalGet", self.GlobalGet);
        push("GlobalSet", self.GlobalSet);
        push("GlobalSetI32Imm16", self.GlobalSetI32Imm16);
        push("GlobalSetI64Imm16", self.GlobalSetI64Imm16);
        push("I32Load", self.I32Load);
        push("I32LoadAt", self.I32LoadAt);
        push("I32LoadOffset16", self.I32LoadOffset16);
        push("I64Load", self.I64Load);
        push("I64LoadAt", self.I64LoadAt);
        push("I64LoadOffset16", self.I64LoadOffset16);
        push("F32Load", self.F32Load);
        push("F32LoadAt", self.F32LoadAt);
        push("F32LoadOffset16", self.F32LoadOffset16);
        push("F64Load", self.F64Load);
        push("F64LoadAt", self.F64LoadAt);
        push("F64LoadOffset16", self.F64LoadOffset16);
        push("I32Load8s", self.I32Load8s);
        push("I32Load8sAt", self.I32Load8sAt);
        push("I32Load8sOffset16", self.I32Load8sOffset16);
        push("I32Load8u", self.I32Load8u);
        push("I32Load8uAt", self.I32Load8uAt);
        push("I32Load8uOffset16", self.I32Load8uOffset16);
        push("I32Load16s", self.I32Load16s);
        push("I32Load16sAt", self.I32Load16sAt);
        push("I32Load16sOffset16", self.I32Load16sOffset16);
        push("I32Load16u", self.I32Load16u);
        push("I32Load16uAt", self.I32Load16uAt);
        push("I32Load16uOffset16", self.I32Load16uOffset16);
        push("I64Load8s", self.I64Load8s);
        push("I64Load8sAt", self.I64Load8sAt);
        push("I64Load8sOffset16", self.I64Load8sOffset16);
        push("I64Load8u", self.I64Load8u);
        push("I64Load8uAt", self.I64Load8uAt);
        push("I64Load8uOffset16", self.I64Load8uOffset16);
        push("I64Load16s", self.I64Load16s);
        push("I64Load16sAt", self.I64Load16sAt);
        push("I64Load16sOffset16", self.I64Load16sOffset16);
        push("I64Load16u", self.I64Load16u);
        push("I64Load16uAt", self.I64Load16uAt);
        push("I64Load16uOffset16", self.I64Load16uOffset16);
        push("I64Load32s", self.I64Load32s);
        push("I64Load32sAt", self.I64Load32sAt);
        push("I64Load32sOffset16", self.I64Load32sOffset16);
        push("I64Load32u", self.I64Load32u);
        push("I64Load32uAt", self.I64Load32uAt);
        push("I64Load32uOffset16", self.I64Load32uOffset16);
        push("I32Store", self.I32Store);
        push("I32StoreOffset16", self.I32StoreOffset16);
        push("I32StoreOffset16Imm16", self.I32StoreOffset16Imm16);
        push("I32StoreAt", self.I32StoreAt);
        push("I32StoreAtImm16", self.I32StoreAtImm16);
        push("I32Store8", self.I32Store8);
        push("I32Store8Offset16", self.I32Store8Offset16);
        push("I32Store8Offset16Imm", self.I32Store8Offset16Imm);
        push("I32Store8At", self.I32Store8At);
        push("I32Store8AtImm", self.I32Store8AtImm);
        push("I32Store16", self.I32Store16);
        push("I32Store16Offset16", self.I32Store16Offset16);
        push("I32Store16Offset16Imm", self.I32Store16Offset16Imm);
        push("I32Store16At", self.I32Store16At);
        push("I32Store16AtImm", self.I32Store16AtImm);
        push("I64Store", self.I64Store);
        push("I64StoreOffset16", self.I64StoreOffset16);
        push("I64StoreOffset16Imm16", self.I64StoreOffset16Imm16);
        push("I64StoreAt", self.I64StoreAt);
        push("I64StoreAtImm16", self.I64StoreAtImm16);
        push("I64Store8", self.I64Store8);
        push("I64Store8Offset16", self.I64Store8Offset16);
        push("I64Store8Offset16Imm", self.I64Store8Offset16Imm);
        push("I64Store8At", self.I64Store8At);
        push("I64Store8AtImm", self.I64Store8AtImm);
        push("I64Store16", self.I64Store16);
        push("I64Store16Offset16", self.I64Store16Offset16);
        push("I64Store16Offset16Imm", self.I64Store16Offset16Imm);
        push("I64Store16At", self.I64Store16At);
        push("I64Store16AtImm", self.I64Store16AtImm);
        push("I64Store32", self.I64Store32);
        push("I64Store32Offset16", self.I64Store32Offset16);
        push("I64Store32Offset16Imm16", self.I64Store32Offset16Imm16);
        push("I64Store32At", self.I64Store32At);
        push("I64Store32AtImm16", self.I64Store32AtImm16);
        push("F32Store", self.F32Store);
        push("F32StoreOffset16", self.F32StoreOffset16);
        push("F32StoreAt", self.F32StoreAt);
        push("F64Store", self.F64Store);
        push("F64StoreOffset16", self.F64StoreOffset16);
        push("F64StoreAt", self.F64StoreAt);
        push("I32Eq", self.I32Eq);
        push("I32EqImm16", self.I32EqImm16);
        push("I64Eq", self.I64Eq);
        push("I64EqImm16", self.I64EqImm16);
        push("I32Ne", self.I32Ne);
        push("I32NeImm16", self.I32NeImm16);
        push("I64Ne", self.I64Ne);
        push("I64NeImm16", self.I64NeImm16);
        push("I32LtS", self.I32LtS);
        push("I32LtU", self.I32LtU);
        push("I32LtSImm16", self.I32LtSImm16);
        push("I32LtUImm16", self.I32LtUImm16);
        push("I64LtS", self.I64LtS);
        push("I64LtU", self.I64LtU);
        push("I64LtSImm16", self.I64LtSImm16);
        push("I64LtUImm16", self.I64LtUImm16);
        push("I32GtS", self.I32GtS);
        push("I32GtU", self.I32GtU);
        push("I32GtSImm16", self.I32GtSImm16);
        push("I32GtUImm16", self.I32GtUImm16);
        push("I64GtS", self.I64GtS);
        push("I64GtU", self.I64GtU);
        push("I64GtSImm16", self.I64GtSImm16);
        push("I64GtUImm16", self.I64GtUImm16);
        push("I32LeS", self.I32LeS);
        push("I32LeU", self.I32LeU);
        push("I32LeSImm16", self.I32LeSImm16);
        push("I32LeUImm16", self.I32LeUImm16);
        push("I64LeS", self.I64LeS);
        push("I64LeU", self.I64LeU);
        push("I64LeSImm16", self.I64LeSImm16);
        push("I64LeUImm16", self.I64LeUImm16);
        push("I32GeS", self.I32GeS);
        push("I32GeU", self.I32GeU);
        push("I32GeSImm16", self.I32GeSImm16);
        push("I32GeUImm16", self.I32GeUImm16);
        push("I64GeS", self.I64GeS);
        push("I64GeU", self.I64GeU);
        push("I64GeSImm16", self.I64GeSImm16);
        push("I64GeUImm16", self.I64GeUImm16);
        push("F32Eq", self.F32Eq);
        push("F64Eq", self.F64Eq);
        push("F32Ne", self.F32Ne);
        push("F64Ne", self.F64Ne);
        push("F32Lt", self.F32Lt);
        push("F64Lt", self.F64Lt);
        push("F32Le", self.F32Le);
        push("F64Le", self.F64Le);
        push("F32Gt", self.F32Gt);
        push("F64Gt", self.F64Gt);
        push("F32Ge", self.F32Ge);
        push("F64Ge", self.F64Ge);
        push("I32Clz", self.I32Clz);
        push("I64Clz", self.I64Clz);
        push("I32Ctz", self.I32Ctz);
        push("I64Ctz", self.I64Ctz);
        push("I32Popcnt", self.I32Popcnt);
        push("I64Popcnt", self.I64Popcnt);
        push("I32Add", self.I32Add);
        push("I64Add", self.I64Add);
        push("I32AddImm16", self.I32AddImm16);
        push("I64AddImm16", self.I64AddImm16);
        push("I32Sub", self.I32Sub);
        push("I64Sub", self.I64Sub);
        push("I32SubImm16", self.I32SubImm16);
        push("I64SubImm16", self.I64SubImm16);
        push("I32SubImm16Rev", self.I32SubImm16Rev);
        push("I64SubImm16Rev", self.I64SubImm16Rev);
        push("I32Mul", self.I32Mul);
        push("I64Mul", self.I64Mul);
        push("I32MulImm16", self.I32MulImm16);
        push("I64MulImm16", self.I64MulImm16);
        push("I32DivS", self.I32DivS);
        push("I64DivS", self.I64DivS);
        push("I32DivSImm16", self.I32DivSImm16);
        push("I64DivSImm16", self.I64DivSImm16);
        push("I32DivSImm16Rev", self.I32DivSImm16Rev);
        push("I64DivSImm16Rev", self.I64DivSImm16Rev);
        push("I32DivU", self.I32DivU);
        push("I64DivU", self.I64DivU);
        push("I32DivUImm16", self.I32DivUImm16);
        push("I64DivUImm16", self.I64DivUImm16);
        push("I32DivUImm16Rev", self.I32DivUImm16Rev);
        push("I64DivUImm16Rev", self.I64DivUImm16Rev);
        push("I32RemS", self.I32RemS);
        push("I64RemS", self.I64RemS);
        push("I32RemSImm16", self.I32RemSImm16);
        push("I64RemSImm16", self.I64RemSImm16);
        push("I32RemSImm16Rev", self.I32RemSImm16Rev);
        push("I64RemSImm16Rev", self.I64RemSImm16Rev);
        push("I32RemU", self.I32RemU);
        push("I64RemU", self.I64RemU);
        push("I32RemUImm16", self.I32RemUImm16);
        push("I64RemUImm16", self.I64RemUImm16);
        push("I32RemUImm16Rev", self.I32RemUImm16Rev);
        push("I64RemUImm16Rev", self.I64RemUImm16Rev);
        push("I32And", self.I32And);
        push("I64And", self.I64And);
        push("I32AndImm16", self.I32AndImm16);
        push("I64AndImm16", self.I64AndImm16);
        push("I32Or", self.I32Or);
        push("I64Or", self.I64Or);
        push("I32OrImm16", self.I32OrImm16);
        push("I64OrImm16", self.I64OrImm16);
        push("I32Xor", self.I32Xor);
        push("I64Xor", self.I64Xor);
        push("I32XorImm16", self.I32XorImm16);
        push("I64XorImm16", self.I64XorImm16);
        push("I32Shl", self.I32Shl);
        push("I64Shl", self.I64Shl);
        push("I32ShlImm", self.I32ShlImm);
        push("I64ShlImm", self.I64ShlImm);
        push("I32ShlImm16Rev", self.I32ShlImm16Rev);
        push("I64ShlImm16Rev", self.I64ShlImm16Rev);
        push("I32ShrU", self.I32ShrU);
        push("I64ShrU", self.I64ShrU);
        push("I32ShrUImm", self.I32ShrUImm);
        push("I64ShrUImm", self.I64ShrUImm);
        push("I32ShrUImm16Rev", self.I32ShrUImm16Rev);
        push("I64ShrUImm16Rev", self.I64ShrUImm16Rev);
        push("I32ShrS", self.I32ShrS);
        push("I64ShrS", self.I64ShrS);
        push("I32ShrSImm", self.I32ShrSImm);
        push("I64ShrSImm", self.I64ShrSImm);
        push("I32ShrSImm16Rev", self.I32ShrSImm16Rev);
        push("I64ShrSImm16Rev", self.I64ShrSImm16Rev);
        push("I32Rotl", self.I32Rotl);
        push("I64Rotl", self.I64Rotl);
        push("I32RotlImm", self.I32RotlImm);
        push("I64RotlImm", self.I64RotlImm);
        push("I32RotlImm16Rev", self.I32RotlImm16Rev);
        push("I64RotlImm16Rev", self.I64RotlImm16Rev);
        push("I32Rotr", self.I32Rotr);
        push("I64Rotr", self.I64Rotr);
        push("I32RotrImm", self.I32RotrImm);
        push("I64RotrImm", self.I64RotrImm);
        push("I32RotrImm16Rev", self.I32RotrImm16Rev);
        push("I64RotrImm16Rev", self.I64RotrImm16Rev);
        push("F32Abs", self.F32Abs);
        push("F64Abs", self.F64Abs);
        push("F32Neg", self.F32Neg);
        push("F64Neg", self.F64Neg);
        push("F32Ceil", self.F32Ceil);
        push("F64Ceil", self.F64Ceil);
        push("F32Floor", self.F32Floor);
        push("F64Floor", self.F64Floor);
        push("F32Trunc", self.F32Trunc);
        push("F64Trunc", self.F64Trunc);
        push("F32Nearest", self.F32Nearest);
        push("F64Nearest", self.F64Nearest);
        push("F32Sqrt", self.F32Sqrt);
        push("F64Sqrt", self.F64Sqrt);
        push("F32Add", self.F32Add);
        push("F64Add", self.F64Add);
        push("F32Sub", self.F32Sub);
        push("F64Sub", self.F64Sub);
        push("F32Mul", self.F32Mul);
        push("F64Mul", self.F64Mul);
        push("F32Div", self.F32Div);
        push("F64Div", self.F64Div);
        push("F32Min", self.F32Min);
        push("F64Min", self.F64Min);
        push("F32Max", self.F32Max);
        push("F64Max", self.F64Max);
        push("F32Copysign", self.F32Copysign);
        push("F64Copysign", self.F64Copysign);
        push("F32CopysignImm", self.F32CopysignImm);
        push("F64CopysignImm", self.F64CopysignImm);
        push("I32WrapI64", self.I32WrapI64);
        push("I64ExtendI32S", self.I64ExtendI32S);
        push("I64ExtendI32U", self.I64ExtendI32U);
        push("I32TruncF32S", self.I32TruncF32S);
        push("I32TruncF32U", self.I32TruncF32U);
        push("I32TruncF64S", self.I32TruncF64S);
        push("I32TruncF64U", self.I32TruncF64U);
        push("I64TruncF32S", self.I64TruncF32S);
        push("I64TruncF32U", self.I64TruncF32U);
        push("I64TruncF64S", self.I64TruncF64S);
        push("I64TruncF64U", self.I64TruncF64U);
        push("I32TruncSatF32S", self.I32TruncSatF32S);
        push("I32TruncSatF32U", self.I32TruncSatF32U);
        push("I32TruncSatF64S", self.I32TruncSatF64S);
        push("I32TruncSatF64U", self.I32TruncSatF64U);
        push("I64TruncSatF32S", self.I64TruncSatF32S);
        push("I64TruncSatF32U", self.I64TruncSatF32U);
        push("I64TruncSatF64S", self.I64TruncSatF64S);
        push("I64TruncSatF64U", self.I64TruncSatF64U);
        push("I32Extend8S", self.I32Extend8S);
        push("I32Extend16S", self.I32Extend16S);
        push("I64Extend8S", self.I64Extend8S);
        push("I64Extend16S", self.I64Extend16S);
        push("I64Extend32S", self.I64Extend32S);
        push("F32DemoteF64", self.F32DemoteF64);
        push("F64PromoteF32", self.F64PromoteF32);
        push("F32ConvertI32S", self.F32ConvertI32S);
        push("F32ConvertI32U", self.F32ConvertI32U);
        push("F32ConvertI64S", self.F32ConvertI64S);
        push("F32ConvertI64U", self.F32ConvertI64U);
        push("F64ConvertI32S", self.F64ConvertI32S);
        push("F64ConvertI32U", self.F64ConvertI32U);
        push("F64ConvertI64S", self.F64ConvertI64S);
        push("F64ConvertI64U", self.F64ConvertI64U);
        buffer.sort_by(|(_ident_a, count_a), (_ident_b, count_b)| {
            count_a.cmp(count_b)
        });
        f.debug_map()
            .entries(buffer)
            .finish()
    }
}

impl InstructionCounts {
    pub fn bump(&mut self, instr: &Instruction) {
        match instr {
            Instruction::TableIdx { .. } => self.TableIdx += 1,
            Instruction::DataSegmentIdx { .. } => self.DataSegmentIdx += 1,
            Instruction::ElementSegmentIdx { .. } => self.ElementSegmentIdx += 1,
            Instruction::Const32 { .. } => self.Const32 += 1,
            Instruction::I64Const32 { .. } => self.I64Const32 += 1,
            Instruction::F64Const32 { .. } => self.F64Const32 += 1,
            Instruction::Register { .. } => self.Register += 1,
            Instruction::Register2 { .. } => self.Register2 += 1,
            Instruction::Register3 { .. } => self.Register3 += 1,
            Instruction::RegisterList { .. } => self.RegisterList += 1,
            Instruction::CallIndirectParams { .. } => self.CallIndirectParams += 1,
            Instruction::CallIndirectParamsImm16 { .. } => self.CallIndirectParamsImm16 += 1,
            Instruction::Trap { .. } => self.Trap += 1,
            Instruction::ConsumeFuel { .. } => self.ConsumeFuel += 1,
            Instruction::Return { .. } => self.Return += 1,
            Instruction::ReturnReg { .. } => self.ReturnReg += 1,
            Instruction::ReturnReg2 { .. } => self.ReturnReg2 += 1,
            Instruction::ReturnReg3 { .. } => self.ReturnReg3 += 1,
            Instruction::ReturnImm32 { .. } => self.ReturnImm32 += 1,
            Instruction::ReturnI64Imm32 { .. } => self.ReturnI64Imm32 += 1,
            Instruction::ReturnF64Imm32 { .. } => self.ReturnF64Imm32 += 1,
            Instruction::ReturnSpan { .. } => self.ReturnSpan += 1,
            Instruction::ReturnMany { .. } => self.ReturnMany += 1,
            Instruction::ReturnNez { .. } => self.ReturnNez += 1,
            Instruction::ReturnNezReg { .. } => self.ReturnNezReg += 1,
            Instruction::ReturnNezReg2 { .. } => self.ReturnNezReg2 += 1,
            Instruction::ReturnNezImm32 { .. } => self.ReturnNezImm32 += 1,
            Instruction::ReturnNezI64Imm32 { .. } => self.ReturnNezI64Imm32 += 1,
            Instruction::ReturnNezF64Imm32 { .. } => self.ReturnNezF64Imm32 += 1,
            Instruction::ReturnNezSpan { .. } => self.ReturnNezSpan += 1,
            Instruction::ReturnNezMany { .. } => self.ReturnNezMany += 1,
            Instruction::Branch { .. } => self.Branch += 1,
            Instruction::BranchEqz { .. } => self.BranchEqz += 1,
            Instruction::BranchNez { .. } => self.BranchNez += 1,
            Instruction::BranchTable { .. } => self.BranchTable += 1,
            Instruction::Copy { .. } => self.Copy += 1,
            Instruction::Copy2 { .. } => self.Copy2 += 1,
            Instruction::CopyImm32 { .. } => self.CopyImm32 += 1,
            Instruction::CopyI64Imm32 { .. } => self.CopyI64Imm32 += 1,
            Instruction::CopyF64Imm32 { .. } => self.CopyF64Imm32 += 1,
            Instruction::CopySpan { .. } => self.CopySpan += 1,
            Instruction::CopySpanNonOverlapping { .. } => self.CopySpanNonOverlapping += 1,
            Instruction::CopyMany { .. } => self.CopyMany += 1,
            Instruction::CopyManyNonOverlapping { .. } => self.CopyManyNonOverlapping += 1,
            Instruction::ReturnCallInternal0 { .. } => self.ReturnCallInternal0 += 1,
            Instruction::ReturnCallInternal { .. } => self.ReturnCallInternal += 1,
            Instruction::ReturnCallImported0 { .. } => self.ReturnCallImported0 += 1,
            Instruction::ReturnCallImported { .. } => self.ReturnCallImported += 1,
            Instruction::ReturnCallIndirect0 { .. } => self.ReturnCallIndirect0 += 1,
            Instruction::ReturnCallIndirect { .. } => self.ReturnCallIndirect += 1,
            Instruction::CallInternal0 { .. } => self.CallInternal0 += 1,
            Instruction::CallInternal { .. } => self.CallInternal += 1,
            Instruction::CallImported0 { .. } => self.CallImported0 += 1,
            Instruction::CallImported { .. } => self.CallImported += 1,
            Instruction::CallIndirect0 { .. } => self.CallIndirect0 += 1,
            Instruction::CallIndirect { .. } => self.CallIndirect += 1,
            Instruction::Select { .. } => self.Select += 1,
            Instruction::SelectRev { .. } => self.SelectRev += 1,
            Instruction::SelectImm32 { .. } => self.SelectImm32 += 1,
            Instruction::SelectI64Imm32 { .. } => self.SelectI64Imm32 += 1,
            Instruction::SelectF64Imm32 { .. } => self.SelectF64Imm32 += 1,
            Instruction::RefFunc { .. } => self.RefFunc += 1,
            Instruction::TableGet { .. } => self.TableGet += 1,
            Instruction::TableGetImm { .. } => self.TableGetImm += 1,
            Instruction::TableSize { .. } => self.TableSize += 1,
            Instruction::TableSet { .. } => self.TableSet += 1,
            Instruction::TableSetAt { .. } => self.TableSetAt += 1,
            Instruction::TableCopy { .. } => self.TableCopy += 1,
            Instruction::TableCopyTo { .. } => self.TableCopyTo += 1,
            Instruction::TableCopyFrom { .. } => self.TableCopyFrom += 1,
            Instruction::TableCopyFromTo { .. } => self.TableCopyFromTo += 1,
            Instruction::TableCopyExact { .. } => self.TableCopyExact += 1,
            Instruction::TableCopyToExact { .. } => self.TableCopyToExact += 1,
            Instruction::TableCopyFromExact { .. } => self.TableCopyFromExact += 1,
            Instruction::TableCopyFromToExact { .. } => self.TableCopyFromToExact += 1,
            Instruction::TableInit { .. } => self.TableInit += 1,
            Instruction::TableInitTo { .. } => self.TableInitTo += 1,
            Instruction::TableInitFrom { .. } => self.TableInitFrom += 1,
            Instruction::TableInitFromTo { .. } => self.TableInitFromTo += 1,
            Instruction::TableInitExact { .. } => self.TableInitExact += 1,
            Instruction::TableInitToExact { .. } => self.TableInitToExact += 1,
            Instruction::TableInitFromExact { .. } => self.TableInitFromExact += 1,
            Instruction::TableInitFromToExact { .. } => self.TableInitFromToExact += 1,
            Instruction::TableFill { .. } => self.TableFill += 1,
            Instruction::TableFillAt { .. } => self.TableFillAt += 1,
            Instruction::TableFillExact { .. } => self.TableFillExact += 1,
            Instruction::TableFillAtExact { .. } => self.TableFillAtExact += 1,
            Instruction::TableGrow { .. } => self.TableGrow += 1,
            Instruction::TableGrowImm { .. } => self.TableGrowImm += 1,
            Instruction::ElemDrop { .. } => self.ElemDrop += 1,
            Instruction::DataDrop { .. } => self.DataDrop += 1,
            Instruction::MemorySize { .. } => self.MemorySize += 1,
            Instruction::MemoryGrow { .. } => self.MemoryGrow += 1,
            Instruction::MemoryGrowBy { .. } => self.MemoryGrowBy += 1,
            Instruction::MemoryCopy { .. } => self.MemoryCopy += 1,
            Instruction::MemoryCopyTo { .. } => self.MemoryCopyTo += 1,
            Instruction::MemoryCopyFrom { .. } => self.MemoryCopyFrom += 1,
            Instruction::MemoryCopyFromTo { .. } => self.MemoryCopyFromTo += 1,
            Instruction::MemoryCopyExact { .. } => self.MemoryCopyExact += 1,
            Instruction::MemoryCopyToExact { .. } => self.MemoryCopyToExact += 1,
            Instruction::MemoryCopyFromExact { .. } => self.MemoryCopyFromExact += 1,
            Instruction::MemoryCopyFromToExact { .. } => self.MemoryCopyFromToExact += 1,
            Instruction::MemoryFill { .. } => self.MemoryFill += 1,
            Instruction::MemoryFillAt { .. } => self.MemoryFillAt += 1,
            Instruction::MemoryFillImm { .. } => self.MemoryFillImm += 1,
            Instruction::MemoryFillExact { .. } => self.MemoryFillExact += 1,
            Instruction::MemoryFillAtImm { .. } => self.MemoryFillAtImm += 1,
            Instruction::MemoryFillAtExact { .. } => self.MemoryFillAtExact += 1,
            Instruction::MemoryFillImmExact { .. } => self.MemoryFillImmExact += 1,
            Instruction::MemoryFillAtImmExact { .. } => self.MemoryFillAtImmExact += 1,
            Instruction::MemoryInit { .. } => self.MemoryInit += 1,
            Instruction::MemoryInitTo { .. } => self.MemoryInitTo += 1,
            Instruction::MemoryInitFrom { .. } => self.MemoryInitFrom += 1,
            Instruction::MemoryInitFromTo { .. } => self.MemoryInitFromTo += 1,
            Instruction::MemoryInitExact { .. } => self.MemoryInitExact += 1,
            Instruction::MemoryInitToExact { .. } => self.MemoryInitToExact += 1,
            Instruction::MemoryInitFromExact { .. } => self.MemoryInitFromExact += 1,
            Instruction::MemoryInitFromToExact { .. } => self.MemoryInitFromToExact += 1,
            Instruction::GlobalGet { .. } => self.GlobalGet += 1,
            Instruction::GlobalSet { .. } => self.GlobalSet += 1,
            Instruction::GlobalSetI32Imm16 { .. } => self.GlobalSetI32Imm16 += 1,
            Instruction::GlobalSetI64Imm16 { .. } => self.GlobalSetI64Imm16 += 1,
            Instruction::I32Load { .. } => self.I32Load += 1,
            Instruction::I32LoadAt { .. } => self.I32LoadAt += 1,
            Instruction::I32LoadOffset16 { .. } => self.I32LoadOffset16 += 1,
            Instruction::I64Load { .. } => self.I64Load += 1,
            Instruction::I64LoadAt { .. } => self.I64LoadAt += 1,
            Instruction::I64LoadOffset16 { .. } => self.I64LoadOffset16 += 1,
            Instruction::F32Load { .. } => self.F32Load += 1,
            Instruction::F32LoadAt { .. } => self.F32LoadAt += 1,
            Instruction::F32LoadOffset16 { .. } => self.F32LoadOffset16 += 1,
            Instruction::F64Load { .. } => self.F64Load += 1,
            Instruction::F64LoadAt { .. } => self.F64LoadAt += 1,
            Instruction::F64LoadOffset16 { .. } => self.F64LoadOffset16 += 1,
            Instruction::I32Load8s { .. } => self.I32Load8s += 1,
            Instruction::I32Load8sAt { .. } => self.I32Load8sAt += 1,
            Instruction::I32Load8sOffset16 { .. } => self.I32Load8sOffset16 += 1,
            Instruction::I32Load8u { .. } => self.I32Load8u += 1,
            Instruction::I32Load8uAt { .. } => self.I32Load8uAt += 1,
            Instruction::I32Load8uOffset16 { .. } => self.I32Load8uOffset16 += 1,
            Instruction::I32Load16s { .. } => self.I32Load16s += 1,
            Instruction::I32Load16sAt { .. } => self.I32Load16sAt += 1,
            Instruction::I32Load16sOffset16 { .. } => self.I32Load16sOffset16 += 1,
            Instruction::I32Load16u { .. } => self.I32Load16u += 1,
            Instruction::I32Load16uAt { .. } => self.I32Load16uAt += 1,
            Instruction::I32Load16uOffset16 { .. } => self.I32Load16uOffset16 += 1,
            Instruction::I64Load8s { .. } => self.I64Load8s += 1,
            Instruction::I64Load8sAt { .. } => self.I64Load8sAt += 1,
            Instruction::I64Load8sOffset16 { .. } => self.I64Load8sOffset16 += 1,
            Instruction::I64Load8u { .. } => self.I64Load8u += 1,
            Instruction::I64Load8uAt { .. } => self.I64Load8uAt += 1,
            Instruction::I64Load8uOffset16 { .. } => self.I64Load8uOffset16 += 1,
            Instruction::I64Load16s { .. } => self.I64Load16s += 1,
            Instruction::I64Load16sAt { .. } => self.I64Load16sAt += 1,
            Instruction::I64Load16sOffset16 { .. } => self.I64Load16sOffset16 += 1,
            Instruction::I64Load16u { .. } => self.I64Load16u += 1,
            Instruction::I64Load16uAt { .. } => self.I64Load16uAt += 1,
            Instruction::I64Load16uOffset16 { .. } => self.I64Load16uOffset16 += 1,
            Instruction::I64Load32s { .. } => self.I64Load32s += 1,
            Instruction::I64Load32sAt { .. } => self.I64Load32sAt += 1,
            Instruction::I64Load32sOffset16 { .. } => self.I64Load32sOffset16 += 1,
            Instruction::I64Load32u { .. } => self.I64Load32u += 1,
            Instruction::I64Load32uAt { .. } => self.I64Load32uAt += 1,
            Instruction::I64Load32uOffset16 { .. } => self.I64Load32uOffset16 += 1,
            Instruction::I32Store { .. } => self.I32Store += 1,
            Instruction::I32StoreOffset16 { .. } => self.I32StoreOffset16 += 1,
            Instruction::I32StoreOffset16Imm16 { .. } => self.I32StoreOffset16Imm16 += 1,
            Instruction::I32StoreAt { .. } => self.I32StoreAt += 1,
            Instruction::I32StoreAtImm16 { .. } => self.I32StoreAtImm16 += 1,
            Instruction::I32Store8 { .. } => self.I32Store8 += 1,
            Instruction::I32Store8Offset16 { .. } => self.I32Store8Offset16 += 1,
            Instruction::I32Store8Offset16Imm { .. } => self.I32Store8Offset16Imm += 1,
            Instruction::I32Store8At { .. } => self.I32Store8At += 1,
            Instruction::I32Store8AtImm { .. } => self.I32Store8AtImm += 1,
            Instruction::I32Store16 { .. } => self.I32Store16 += 1,
            Instruction::I32Store16Offset16 { .. } => self.I32Store16Offset16 += 1,
            Instruction::I32Store16Offset16Imm { .. } => self.I32Store16Offset16Imm += 1,
            Instruction::I32Store16At { .. } => self.I32Store16At += 1,
            Instruction::I32Store16AtImm { .. } => self.I32Store16AtImm += 1,
            Instruction::I64Store { .. } => self.I64Store += 1,
            Instruction::I64StoreOffset16 { .. } => self.I64StoreOffset16 += 1,
            Instruction::I64StoreOffset16Imm16 { .. } => self.I64StoreOffset16Imm16 += 1,
            Instruction::I64StoreAt { .. } => self.I64StoreAt += 1,
            Instruction::I64StoreAtImm16 { .. } => self.I64StoreAtImm16 += 1,
            Instruction::I64Store8 { .. } => self.I64Store8 += 1,
            Instruction::I64Store8Offset16 { .. } => self.I64Store8Offset16 += 1,
            Instruction::I64Store8Offset16Imm { .. } => self.I64Store8Offset16Imm += 1,
            Instruction::I64Store8At { .. } => self.I64Store8At += 1,
            Instruction::I64Store8AtImm { .. } => self.I64Store8AtImm += 1,
            Instruction::I64Store16 { .. } => self.I64Store16 += 1,
            Instruction::I64Store16Offset16 { .. } => self.I64Store16Offset16 += 1,
            Instruction::I64Store16Offset16Imm { .. } => self.I64Store16Offset16Imm += 1,
            Instruction::I64Store16At { .. } => self.I64Store16At += 1,
            Instruction::I64Store16AtImm { .. } => self.I64Store16AtImm += 1,
            Instruction::I64Store32 { .. } => self.I64Store32 += 1,
            Instruction::I64Store32Offset16 { .. } => self.I64Store32Offset16 += 1,
            Instruction::I64Store32Offset16Imm16 { .. } => self.I64Store32Offset16Imm16 += 1,
            Instruction::I64Store32At { .. } => self.I64Store32At += 1,
            Instruction::I64Store32AtImm16 { .. } => self.I64Store32AtImm16 += 1,
            Instruction::F32Store { .. } => self.F32Store += 1,
            Instruction::F32StoreOffset16 { .. } => self.F32StoreOffset16 += 1,
            Instruction::F32StoreAt { .. } => self.F32StoreAt += 1,
            Instruction::F64Store { .. } => self.F64Store += 1,
            Instruction::F64StoreOffset16 { .. } => self.F64StoreOffset16 += 1,
            Instruction::F64StoreAt { .. } => self.F64StoreAt += 1,
            Instruction::I32Eq { .. } => self.I32Eq += 1,
            Instruction::I32EqImm16 { .. } => self.I32EqImm16 += 1,
            Instruction::I64Eq { .. } => self.I64Eq += 1,
            Instruction::I64EqImm16 { .. } => self.I64EqImm16 += 1,
            Instruction::I32Ne { .. } => self.I32Ne += 1,
            Instruction::I32NeImm16 { .. } => self.I32NeImm16 += 1,
            Instruction::I64Ne { .. } => self.I64Ne += 1,
            Instruction::I64NeImm16 { .. } => self.I64NeImm16 += 1,
            Instruction::I32LtS { .. } => self.I32LtS += 1,
            Instruction::I32LtU { .. } => self.I32LtU += 1,
            Instruction::I32LtSImm16 { .. } => self.I32LtSImm16 += 1,
            Instruction::I32LtUImm16 { .. } => self.I32LtUImm16 += 1,
            Instruction::I64LtS { .. } => self.I64LtS += 1,
            Instruction::I64LtU { .. } => self.I64LtU += 1,
            Instruction::I64LtSImm16 { .. } => self.I64LtSImm16 += 1,
            Instruction::I64LtUImm16 { .. } => self.I64LtUImm16 += 1,
            Instruction::I32GtS { .. } => self.I32GtS += 1,
            Instruction::I32GtU { .. } => self.I32GtU += 1,
            Instruction::I32GtSImm16 { .. } => self.I32GtSImm16 += 1,
            Instruction::I32GtUImm16 { .. } => self.I32GtUImm16 += 1,
            Instruction::I64GtS { .. } => self.I64GtS += 1,
            Instruction::I64GtU { .. } => self.I64GtU += 1,
            Instruction::I64GtSImm16 { .. } => self.I64GtSImm16 += 1,
            Instruction::I64GtUImm16 { .. } => self.I64GtUImm16 += 1,
            Instruction::I32LeS { .. } => self.I32LeS += 1,
            Instruction::I32LeU { .. } => self.I32LeU += 1,
            Instruction::I32LeSImm16 { .. } => self.I32LeSImm16 += 1,
            Instruction::I32LeUImm16 { .. } => self.I32LeUImm16 += 1,
            Instruction::I64LeS { .. } => self.I64LeS += 1,
            Instruction::I64LeU { .. } => self.I64LeU += 1,
            Instruction::I64LeSImm16 { .. } => self.I64LeSImm16 += 1,
            Instruction::I64LeUImm16 { .. } => self.I64LeUImm16 += 1,
            Instruction::I32GeS { .. } => self.I32GeS += 1,
            Instruction::I32GeU { .. } => self.I32GeU += 1,
            Instruction::I32GeSImm16 { .. } => self.I32GeSImm16 += 1,
            Instruction::I32GeUImm16 { .. } => self.I32GeUImm16 += 1,
            Instruction::I64GeS { .. } => self.I64GeS += 1,
            Instruction::I64GeU { .. } => self.I64GeU += 1,
            Instruction::I64GeSImm16 { .. } => self.I64GeSImm16 += 1,
            Instruction::I64GeUImm16 { .. } => self.I64GeUImm16 += 1,
            Instruction::F32Eq { .. } => self.F32Eq += 1,
            Instruction::F64Eq { .. } => self.F64Eq += 1,
            Instruction::F32Ne { .. } => self.F32Ne += 1,
            Instruction::F64Ne { .. } => self.F64Ne += 1,
            Instruction::F32Lt { .. } => self.F32Lt += 1,
            Instruction::F64Lt { .. } => self.F64Lt += 1,
            Instruction::F32Le { .. } => self.F32Le += 1,
            Instruction::F64Le { .. } => self.F64Le += 1,
            Instruction::F32Gt { .. } => self.F32Gt += 1,
            Instruction::F64Gt { .. } => self.F64Gt += 1,
            Instruction::F32Ge { .. } => self.F32Ge += 1,
            Instruction::F64Ge { .. } => self.F64Ge += 1,
            Instruction::I32Clz { .. } => self.I32Clz += 1,
            Instruction::I64Clz { .. } => self.I64Clz += 1,
            Instruction::I32Ctz { .. } => self.I32Ctz += 1,
            Instruction::I64Ctz { .. } => self.I64Ctz += 1,
            Instruction::I32Popcnt { .. } => self.I32Popcnt += 1,
            Instruction::I64Popcnt { .. } => self.I64Popcnt += 1,
            Instruction::I32Add { .. } => self.I32Add += 1,
            Instruction::I64Add { .. } => self.I64Add += 1,
            Instruction::I32AddImm16 { .. } => self.I32AddImm16 += 1,
            Instruction::I64AddImm16 { .. } => self.I64AddImm16 += 1,
            Instruction::I32Sub { .. } => self.I32Sub += 1,
            Instruction::I64Sub { .. } => self.I64Sub += 1,
            Instruction::I32SubImm16 { .. } => self.I32SubImm16 += 1,
            Instruction::I64SubImm16 { .. } => self.I64SubImm16 += 1,
            Instruction::I32SubImm16Rev { .. } => self.I32SubImm16Rev += 1,
            Instruction::I64SubImm16Rev { .. } => self.I64SubImm16Rev += 1,
            Instruction::I32Mul { .. } => self.I32Mul += 1,
            Instruction::I64Mul { .. } => self.I64Mul += 1,
            Instruction::I32MulImm16 { .. } => self.I32MulImm16 += 1,
            Instruction::I64MulImm16 { .. } => self.I64MulImm16 += 1,
            Instruction::I32DivS { .. } => self.I32DivS += 1,
            Instruction::I64DivS { .. } => self.I64DivS += 1,
            Instruction::I32DivSImm16 { .. } => self.I32DivSImm16 += 1,
            Instruction::I64DivSImm16 { .. } => self.I64DivSImm16 += 1,
            Instruction::I32DivSImm16Rev { .. } => self.I32DivSImm16Rev += 1,
            Instruction::I64DivSImm16Rev { .. } => self.I64DivSImm16Rev += 1,
            Instruction::I32DivU { .. } => self.I32DivU += 1,
            Instruction::I64DivU { .. } => self.I64DivU += 1,
            Instruction::I32DivUImm16 { .. } => self.I32DivUImm16 += 1,
            Instruction::I64DivUImm16 { .. } => self.I64DivUImm16 += 1,
            Instruction::I32DivUImm16Rev { .. } => self.I32DivUImm16Rev += 1,
            Instruction::I64DivUImm16Rev { .. } => self.I64DivUImm16Rev += 1,
            Instruction::I32RemS { .. } => self.I32RemS += 1,
            Instruction::I64RemS { .. } => self.I64RemS += 1,
            Instruction::I32RemSImm16 { .. } => self.I32RemSImm16 += 1,
            Instruction::I64RemSImm16 { .. } => self.I64RemSImm16 += 1,
            Instruction::I32RemSImm16Rev { .. } => self.I32RemSImm16Rev += 1,
            Instruction::I64RemSImm16Rev { .. } => self.I64RemSImm16Rev += 1,
            Instruction::I32RemU { .. } => self.I32RemU += 1,
            Instruction::I64RemU { .. } => self.I64RemU += 1,
            Instruction::I32RemUImm16 { .. } => self.I32RemUImm16 += 1,
            Instruction::I64RemUImm16 { .. } => self.I64RemUImm16 += 1,
            Instruction::I32RemUImm16Rev { .. } => self.I32RemUImm16Rev += 1,
            Instruction::I64RemUImm16Rev { .. } => self.I64RemUImm16Rev += 1,
            Instruction::I32And { .. } => self.I32And += 1,
            Instruction::I64And { .. } => self.I64And += 1,
            Instruction::I32AndImm16 { .. } => self.I32AndImm16 += 1,
            Instruction::I64AndImm16 { .. } => self.I64AndImm16 += 1,
            Instruction::I32Or { .. } => self.I32Or += 1,
            Instruction::I64Or { .. } => self.I64Or += 1,
            Instruction::I32OrImm16 { .. } => self.I32OrImm16 += 1,
            Instruction::I64OrImm16 { .. } => self.I64OrImm16 += 1,
            Instruction::I32Xor { .. } => self.I32Xor += 1,
            Instruction::I64Xor { .. } => self.I64Xor += 1,
            Instruction::I32XorImm16 { .. } => self.I32XorImm16 += 1,
            Instruction::I64XorImm16 { .. } => self.I64XorImm16 += 1,
            Instruction::I32Shl { .. } => self.I32Shl += 1,
            Instruction::I64Shl { .. } => self.I64Shl += 1,
            Instruction::I32ShlImm { .. } => self.I32ShlImm += 1,
            Instruction::I64ShlImm { .. } => self.I64ShlImm += 1,
            Instruction::I32ShlImm16Rev { .. } => self.I32ShlImm16Rev += 1,
            Instruction::I64ShlImm16Rev { .. } => self.I64ShlImm16Rev += 1,
            Instruction::I32ShrU { .. } => self.I32ShrU += 1,
            Instruction::I64ShrU { .. } => self.I64ShrU += 1,
            Instruction::I32ShrUImm { .. } => self.I32ShrUImm += 1,
            Instruction::I64ShrUImm { .. } => self.I64ShrUImm += 1,
            Instruction::I32ShrUImm16Rev { .. } => self.I32ShrUImm16Rev += 1,
            Instruction::I64ShrUImm16Rev { .. } => self.I64ShrUImm16Rev += 1,
            Instruction::I32ShrS { .. } => self.I32ShrS += 1,
            Instruction::I64ShrS { .. } => self.I64ShrS += 1,
            Instruction::I32ShrSImm { .. } => self.I32ShrSImm += 1,
            Instruction::I64ShrSImm { .. } => self.I64ShrSImm += 1,
            Instruction::I32ShrSImm16Rev { .. } => self.I32ShrSImm16Rev += 1,
            Instruction::I64ShrSImm16Rev { .. } => self.I64ShrSImm16Rev += 1,
            Instruction::I32Rotl { .. } => self.I32Rotl += 1,
            Instruction::I64Rotl { .. } => self.I64Rotl += 1,
            Instruction::I32RotlImm { .. } => self.I32RotlImm += 1,
            Instruction::I64RotlImm { .. } => self.I64RotlImm += 1,
            Instruction::I32RotlImm16Rev { .. } => self.I32RotlImm16Rev += 1,
            Instruction::I64RotlImm16Rev { .. } => self.I64RotlImm16Rev += 1,
            Instruction::I32Rotr { .. } => self.I32Rotr += 1,
            Instruction::I64Rotr { .. } => self.I64Rotr += 1,
            Instruction::I32RotrImm { .. } => self.I32RotrImm += 1,
            Instruction::I64RotrImm { .. } => self.I64RotrImm += 1,
            Instruction::I32RotrImm16Rev { .. } => self.I32RotrImm16Rev += 1,
            Instruction::I64RotrImm16Rev { .. } => self.I64RotrImm16Rev += 1,
            Instruction::F32Abs { .. } => self.F32Abs += 1,
            Instruction::F64Abs { .. } => self.F64Abs += 1,
            Instruction::F32Neg { .. } => self.F32Neg += 1,
            Instruction::F64Neg { .. } => self.F64Neg += 1,
            Instruction::F32Ceil { .. } => self.F32Ceil += 1,
            Instruction::F64Ceil { .. } => self.F64Ceil += 1,
            Instruction::F32Floor { .. } => self.F32Floor += 1,
            Instruction::F64Floor { .. } => self.F64Floor += 1,
            Instruction::F32Trunc { .. } => self.F32Trunc += 1,
            Instruction::F64Trunc { .. } => self.F64Trunc += 1,
            Instruction::F32Nearest { .. } => self.F32Nearest += 1,
            Instruction::F64Nearest { .. } => self.F64Nearest += 1,
            Instruction::F32Sqrt { .. } => self.F32Sqrt += 1,
            Instruction::F64Sqrt { .. } => self.F64Sqrt += 1,
            Instruction::F32Add { .. } => self.F32Add += 1,
            Instruction::F64Add { .. } => self.F64Add += 1,
            Instruction::F32Sub { .. } => self.F32Sub += 1,
            Instruction::F64Sub { .. } => self.F64Sub += 1,
            Instruction::F32Mul { .. } => self.F32Mul += 1,
            Instruction::F64Mul { .. } => self.F64Mul += 1,
            Instruction::F32Div { .. } => self.F32Div += 1,
            Instruction::F64Div { .. } => self.F64Div += 1,
            Instruction::F32Min { .. } => self.F32Min += 1,
            Instruction::F64Min { .. } => self.F64Min += 1,
            Instruction::F32Max { .. } => self.F32Max += 1,
            Instruction::F64Max { .. } => self.F64Max += 1,
            Instruction::F32Copysign { .. } => self.F32Copysign += 1,
            Instruction::F64Copysign { .. } => self.F64Copysign += 1,
            Instruction::F32CopysignImm { .. } => self.F32CopysignImm += 1,
            Instruction::F64CopysignImm { .. } => self.F64CopysignImm += 1,
            Instruction::I32WrapI64 { .. } => self.I32WrapI64 += 1,
            Instruction::I64ExtendI32S { .. } => self.I64ExtendI32S += 1,
            Instruction::I64ExtendI32U { .. } => self.I64ExtendI32U += 1,
            Instruction::I32TruncF32S { .. } => self.I32TruncF32S += 1,
            Instruction::I32TruncF32U { .. } => self.I32TruncF32U += 1,
            Instruction::I32TruncF64S { .. } => self.I32TruncF64S += 1,
            Instruction::I32TruncF64U { .. } => self.I32TruncF64U += 1,
            Instruction::I64TruncF32S { .. } => self.I64TruncF32S += 1,
            Instruction::I64TruncF32U { .. } => self.I64TruncF32U += 1,
            Instruction::I64TruncF64S { .. } => self.I64TruncF64S += 1,
            Instruction::I64TruncF64U { .. } => self.I64TruncF64U += 1,
            Instruction::I32TruncSatF32S { .. } => self.I32TruncSatF32S += 1,
            Instruction::I32TruncSatF32U { .. } => self.I32TruncSatF32U += 1,
            Instruction::I32TruncSatF64S { .. } => self.I32TruncSatF64S += 1,
            Instruction::I32TruncSatF64U { .. } => self.I32TruncSatF64U += 1,
            Instruction::I64TruncSatF32S { .. } => self.I64TruncSatF32S += 1,
            Instruction::I64TruncSatF32U { .. } => self.I64TruncSatF32U += 1,
            Instruction::I64TruncSatF64S { .. } => self.I64TruncSatF64S += 1,
            Instruction::I64TruncSatF64U { .. } => self.I64TruncSatF64U += 1,
            Instruction::I32Extend8S { .. } => self.I32Extend8S += 1,
            Instruction::I32Extend16S { .. } => self.I32Extend16S += 1,
            Instruction::I64Extend8S { .. } => self.I64Extend8S += 1,
            Instruction::I64Extend16S { .. } => self.I64Extend16S += 1,
            Instruction::I64Extend32S { .. } => self.I64Extend32S += 1,
            Instruction::F32DemoteF64 { .. } => self.F32DemoteF64 += 1,
            Instruction::F64PromoteF32 { .. } => self.F64PromoteF32 += 1,
            Instruction::F32ConvertI32S { .. } => self.F32ConvertI32S += 1,
            Instruction::F32ConvertI32U { .. } => self.F32ConvertI32U += 1,
            Instruction::F32ConvertI64S { .. } => self.F32ConvertI64S += 1,
            Instruction::F32ConvertI64U { .. } => self.F32ConvertI64U += 1,
            Instruction::F64ConvertI32S { .. } => self.F64ConvertI32S += 1,
            Instruction::F64ConvertI32U { .. } => self.F64ConvertI32U += 1,
            Instruction::F64ConvertI64S { .. } => self.F64ConvertI64S += 1,
            Instruction::F64ConvertI64U { .. } => self.F64ConvertI64U += 1,
        }
    }
}
