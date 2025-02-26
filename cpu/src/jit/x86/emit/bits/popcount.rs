use crate::jit::api::JITResult;
use crate::jit::x86::emit::slice_load::CopyExtendMode;
use crate::jit::x86::regalloc::SliceType;
use crate::jit::x86::{
    X86MogiJit,
    assemble,
};
use mogi_sleigh::spec::jir::values::{
    JIRSlice,
    JIRValue,
};

impl<PageDataT> X86MogiJit<PageDataT> {
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
            1 => {
                self.copy_gpr_to_gpr(out_gpr, input_gpr, 1, CopyExtendMode::ZeroExtend(2))?;
                assemble!(self.block_mut()?.assembler();
                    ; popcnt Rw(out_gpr as u8), Rw(out_gpr as u8)
                );
            }
            2 => {
                assemble!(self.block_mut()?.assembler();
                    ; popcnt Rw(out_gpr as u8), Rw(input_gpr as u8)
                );
            }
            4 => {
                assemble!(self.block_mut()?.assembler();
                    ; popcnt Rd(out_gpr as u8), Rd(input_gpr as u8)
                );
            }
            8 => {
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
