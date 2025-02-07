use crate::jit::api::JITResult;
use crate::jit::x86::X86MogiJit;
use crate::jit::x86::emit::slice_load::CopyExtendMode;
use crate::jit::x86::regalloc::SliceType;
use mogi_sleigh::spec::jir::values::{
    JIRSlice,
    JIRValue,
};

impl X86MogiJit {
    pub fn emit_zero_extend(
        &mut self,
        output: JIRSlice,
        input: JIRValue,
    ) -> JITResult {
        match input {
            JIRValue::Slice(slice) => {
                let in_gpr = self.load_slice_gpr(slice, SliceType::Input);
                let out_gpr = self.get_slice_gp(output, SliceType::Output);
                self.copy_gpr_to_gpr(out_gpr, in_gpr, input.size(), CopyExtendMode::ZeroExtend(output.size()))?;
            }
            JIRValue::Const(value) => {
                // For const values we simply copy
                let value = value.zext(output.size()).expect("Invalid zero extend");
                self.emit_copy(output, value)?;
            }
        }
        Ok(())
    }
}
