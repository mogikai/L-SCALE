use crate::jit::api::{
    JITResult,
    JitError,
};
use crate::jit::x86::emit::arithmetic::integer::AddSub;
use crate::jit::x86::regalloc::SliceType;
use crate::jit::x86::{
    X86MogiJit,
    assemble,
};
use mogi_sleigh::spec::jir::common::JIRSize;
use mogi_sleigh::spec::jir::values::{
    JIRConst,
    JIRSlice,
    JIRValue,
};

impl X86MogiJit {
    #[inline]
    pub fn emit_subtract(
        &mut self,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        debug_assert_eq!(output.size(), input_a.size());
        debug_assert_eq!(input_a.size(), input_b.size());
        let size = output.size();
        match (input_a, input_b) {
            (JIRValue::Const(input_a), JIRValue::Const(input_b)) => {
                self.emit_sub_const_const(output, input_a, input_b)?;
            }
            (JIRValue::Slice(input_a), JIRValue::Const(input_b)) => {
                self.emit_add_sub_reg_const(output, input_a, input_b.value(), AddSub::Sub)?;
            }
            (JIRValue::Const(input_a), JIRValue::Slice(input_b)) => {
                self.emit_sub_const_reg(output, input_a, input_b)?;
            }
            (JIRValue::Slice(input_a), JIRValue::Slice(input_b)) => {
                self.emit_sub_slice_slice(output, size, input_a, input_b)?;
            }
        }
        Ok(())
    }

    #[inline]
    fn emit_sub_const_const(
        &mut self,
        output: JIRSlice,
        input_a: JIRConst,
        input_b: JIRConst,
    ) -> Result<(), JitError> {
        let size = output.size();
        self.emit_copy(output, JIRConst::new((input_a.value() - input_b.value()) % (1u128 << (size * 8)), size))?;
        Ok(())
    }

    #[inline]
    fn emit_sub_slice_slice(
        &mut self,
        output: JIRSlice,
        size: JIRSize,
        input_a: JIRSlice,
        input_b: JIRSlice,
    ) -> Result<(), JitError> {
        if input_a == input_b {
            // A-A = 0, so we can just zero out the output.
            self.emit_copy(output, JIRConst::new(0, size))?;
        } else if output == input_b {
            let temp_gp = self.get_temp_gpr();
            // Move output=input_b to a temporary register.
            self.emit_load_slice_to_gpr(temp_gp, output, SliceType::Input)?;
            // Move input_a to output (possibly loading the slice if it is still unloaded)
            self.emit_copy(output, input_a)?;
            // Get the output gp register (it is loaded, since we just copied to it).
            let output_gp = self.get_slice_gp(output, SliceType::Output);
            // Subtract input_b=output ,which is stored in the temporary register, from the output (output=input_a).
            assemble!(self.block_mut()?.assembler();
                ; sub Rq(output_gp as u8), Rq(temp_gp as u8)
            );
        } else {
            // Copy input_a into output. If output == input_a we can just load output.
            if output != input_a {
                self.emit_copy(output, input_b)?;
            };
            let output_gp = self.load_slice_gpr(output, SliceType::Output);
            let input_b_gp = self.load_slice_gpr(input_b, SliceType::Input);
            // We can just subtract from output
            assemble!(self.block_mut()?.assembler();
                ; sub Rq(output_gp as u8), Rq(input_b_gp as u8)
            );
        }
        Ok(())
    }

    /// Emit instructions to subtract the provided [JIRSlice] `input_b` from the provided [JIRConst] `input_a`.
    /// The result is stored in the [JIRSlice] `output`.
    #[inline]
    fn emit_sub_const_reg(
        &mut self,
        output: JIRSlice,
        input_a: JIRConst,
        input_b: JIRSlice,
    ) -> JITResult {
        if output != input_b {
            // If the output register does not appear on the rhs, we can use it to load the constant
            // We use get_slice_gp here, since the value is overwritten anyway. No need to load here.
            let output_gp = self.get_slice_gp(output, SliceType::Output);
            let input_b_gp = self.load_slice_gpr(input_b, SliceType::Input);
            self.emit_load_const_to_gpr(output_gp, input_a)?;
            // and then just subtract the operand register from the constant in the output register.
            assemble!(self.block_mut()?.assembler();
                ; sub Rq(output_gp as u8), Rq(input_b_gp as u8)
            );
        } else {
            let temp_gp = self.get_temp_gpr();
            // Output appears on rhs (out = const - out)
            // Move the output into a temporary register
            self.emit_load_slice_to_gpr(temp_gp, output, SliceType::Input)?;
            // Load the constant into the output
            self.emit_copy(output, input_a)?;
            // Get the output gp register.
            let output_gp = self.get_slice_gp(output, SliceType::Output);
            // Subtract input_b=output ,which is stored in the temporary register, from the output (output=const).
            assemble!(self.block_mut()?.assembler();
                ; sub Rq(output_gp as u8), Rq(temp_gp as u8)
            );
        }
        Ok(())
    }
}
