use crate::jit::api::JITResult;
use crate::jit::x86::assemble;
use crate::jit::x86::x64emitter::X64Emitter;
use crate::jit::x86::x64emitter::regalloc::SliceType;
use crate::jit::x86::x64emitter::slice_load::CopyExtendMode;
use mogi_sleigh::spec::jir::values::{
    JIRSlice,
    JIRValue,
};

impl X64Emitter<'_> {
    pub fn emit_lzcount(
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
                let correction = -8i32;
                // We need to mask out the first 8 bits and then correct for these bits.
                self.copy_gpr_to_gpr(out_gpr, input_gpr, 1, CopyExtendMode::ZeroExtend(2))?;
                assemble!(self.block.assembler();
                    ; lzcnt Rw(out_gpr as u8), Rw(input_gpr as u8)
                    ; lea Rw(out_gpr as u8), [Rq(out_gpr as u8) + correction]
                );
            }
            2 => {
                assemble!(self.block.assembler();
                    ; lzcnt Rw(out_gpr as u8), Rw(input_gpr as u8)
                );
            }
            4 => {
                assemble!(self.block.assembler();
                    ; lzcnt Rd(out_gpr as u8), Rd(input_gpr as u8)
                );
            }
            8 => {
                assemble!(self.block.assembler();
                    ; lzcnt Rq(out_gpr as u8), Rq(input_gpr as u8)
                );
            }
            _ => todo!("Unsupported size"),
        }
        self.temp_free(input_gpr)?;
        Ok(())
    }
}
