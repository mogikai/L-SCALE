use crate::jit::api::JITResult;
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

        // We want to directly load the input into the output and then shift the output.
        // The x86 shl operation operates in place.
        let out_gpr = self.get_slice_gp(output, SliceType::Output);
        self.emit_load_value_to_gpr(out_gpr, input)?;
        match shift {
            JIRValue::Slice(slice) => {
                // shl expects its argument in the CL register.
                self.load_slice_to_gpr(GPRegister::CX, slice, SliceType::Input)?;
                // we use the 64 bit register here and just discard/ignore the upper bits for simplicity.
                assemble!(self.block_mut()?.assembler();
                    ; shl Rq(out_gpr as u8), cl
                )
            }
            JIRValue::Const(value) => {
                // We simply cut off the shift amount over 8 bits because it would be meaningless to shift more
                // This is only the case for slices that are <= 8 bytes.
                let shift = value.value() as i8;
                assemble!(self.block_mut()?.assembler();
                    ; shl Rq(out_gpr as u8), shift
                );
            }
        }
        Ok(())
    }
}
