use crate::jit::api::JITResult;
use crate::jit::trace_jit;
use crate::jit::x86::regalloc::SliceType;
use crate::jit::x86::registers::GPRegister;
use crate::jit::x86::{
    X86MogiJit,
    assemble,
};
use mogi_sleigh::spec::jir::values::{
    JIRConst,
    JIRSlice,
    JIRValue,
};

impl X86MogiJit {
    pub fn emit_shift_left(
        &mut self,
        output: JIRSlice,
        input: JIRValue,
        shift: JIRValue,
    ) -> JITResult {
        assert_eq!(output.size(), input.size());
        if output.size() > 8 {
            todo!("Sizes greater than 8 are currently unsupported for shift operations.")
        }
        trace_jit!("Emitting shl {output:?} = {input:?} << {shift:?}");

        match shift {
            JIRValue::Slice(shift) => {
                // shl expects its argument in the CL register.
                self.load_slice_gpr_exact(GPRegister::CX, shift, SliceType::Input)?;
                // We want to directly load the input into the output and then shift the output.
                // The x86 shl operation operates in place.
                self.emit_copy(output, input)?;
                let out_gpr = self.load_slice_gpr(output, SliceType::Output);
                // we use the 64 bit register here and just discard/ignore the upper bits for simplicity.
                assemble!(self.block_mut()?.assembler();
                    ; shl Rq(out_gpr as u8), cl
                )
            }
            JIRValue::Const(shift) => {
                // We simply cut off the shift amount over 8 bits because it would be meaningless to shift more
                // This is only the case for slices that are <= 8 bytes.
                let shift = shift.value() as i8;
                // We want to directly load the input into the output and then shift the output.
                // The x86 shl operation operates in place.
                self.emit_copy(output, input)?;
                let out_gpr = self.load_slice_gpr(output, SliceType::Output);
                assemble!(self.block_mut()?.assembler();
                    ; shl Rq(out_gpr as u8), shift
                );
            }
        }
        Ok(())
    }
}
