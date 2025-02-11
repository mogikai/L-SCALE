use crate::jit::api::JITResult;
use crate::jit::x86::regalloc::SliceType;
use crate::jit::x86::{
    X86MogiJit,
    assemble,
};
use mogi_sleigh::spec::jir::values::{
    JIRSlice,
    JIRValue,
};

impl X86MogiJit {
    pub fn emit_popcount(
        &mut self,
        output: JIRSlice,
        input: JIRValue,
    ) -> JITResult {
        let input_gpr = match input {
            JIRValue::Slice(input) => self.load_slice_gpr(input, SliceType::Input),
            JIRValue::Const(input) => {
                let out_gpr = self.temp_alloc();
                self.emit_load_const_to_gpr(out_gpr, input)?;
                out_gpr
            }
        };
        let out_gpr = self.allocate_slice_gpr(output, SliceType::Output);
        match input.size() {
            ..=4 => {
                assemble!(self.block_mut()?.assembler();
                    ; popcnt Rd(out_gpr as u8), Rd(input_gpr as u8)
                );
            }
            ..=8 => {
                assemble!(self.block_mut()?.assembler();
                    ; popcnt Rq(out_gpr as u8), Rq(input_gpr as u8)
                );
            }
            _ => todo!("Unsupported size"),
        }
        self.temp_free(input_gpr)?;
        Ok(())
    }
}
