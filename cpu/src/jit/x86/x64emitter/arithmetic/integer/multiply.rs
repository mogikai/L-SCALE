use crate::jit::api::{
    JITResult,
    JitError,
};
use crate::jit::x86::assemble;
use crate::jit::x86::x64emitter::X64Emitter;
use crate::jit::x86::x64emitter::regalloc::SliceType;
use mogi_sleigh::spec::jir::values::{
    JIRConst,
    JIRSlice,
    JIRValue,
};
use std::cmp::min;

impl X64Emitter<'_> {
    #[inline]
    pub fn emit_multiply(
        &mut self,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        match (input_a, input_b) {
            (JIRValue::Const(input_a), JIRValue::Const(input_b)) => {
                self.emit_multiply_const_const(output, input_a, input_b)?;
            }
            (JIRValue::Slice(input_a), JIRValue::Const(input_b))
            | (JIRValue::Const(input_b), JIRValue::Slice(input_a)) => {
                // Since multiplication is commutative, we can just swap the slice and const arguments
                self.emit_multiply_slice_const(output, input_a, input_b)?;
            }
            (JIRValue::Slice(input_a), JIRValue::Slice(input_b)) => {
                self.emit_multiply_slice_slice(output, input_a, input_b)?;
            }
        }
        Ok(())
    }

    #[inline]
    fn emit_multiply_const_const(
        &mut self,
        output: JIRSlice,
        input_a: JIRConst,
        input_b: JIRConst,
    ) -> Result<(), JitError> {
        let size = output.size();
        self.emit_copy(
            output,
            JIRConst::new(input_a.value().wrapping_mul(input_b.value()) % (1u128 << (size * 8)), size),
        )?;
        Ok(())
    }

    #[inline]
    fn emit_multiply_slice_const(
        &mut self,
        output: JIRSlice,
        input_a: JIRSlice,
        input_b: JIRConst,
    ) -> Result<(), JitError> {
        if input_b.value() == 0 {
            // We can just zero the target, since multiplying by zero always results in zero.
            self.emit_copy(output, input_b)?;
            return Ok(());
        }
        // The size in bytes needed for the immediate
        let imm_size = min((u128::BITS - input_b.value().leading_zeros() + 7) / 8, input_b.size().into());
        match imm_size {
            1 => {
                // We can encode the constant as immediate if it fits into at most 4  bytes.
                let output_gp = self.load_slice_gpr(output, SliceType::Output);
                let input_a_gp = self.load_slice_gpr(input_a, SliceType::Input);
                let immediate = input_b.value();
                assemble!(self.block.assembler();
                    ; imul Rq(output_gp as u8), Rq(input_a_gp as u8), BYTE immediate as _
                );
            }
            2 => {
                // We can encode the constant as immediate if it fits into at most 4  bytes.
                let output_gp = self.load_slice_gpr(output, SliceType::Output);
                let input_a_gp = self.load_slice_gpr(input_a, SliceType::Input);
                let immediate = input_b.value();
                assemble!(self.block.assembler();
                    ; imul Rq(output_gp as u8), Rq(input_a_gp as u8), WORD immediate as _
                );
            }
            3..=4 => {
                // We can encode the constant as immediate if it fits into at most 4  bytes.
                let output_gp = self.load_slice_gpr(output, SliceType::Output);
                let input_a_gp = self.load_slice_gpr(input_a, SliceType::Input);
                let immediate = input_b.value();
                assemble!(self.block.assembler();
                    ; imul Rq(output_gp as u8), Rq(input_a_gp as u8), DWORD immediate as _
                );
            }
            5..=8 => {
                // We need to copy the input to the output, since imul reg, reg uses the first argument as
                // input and output argument.
                self.emit_copy(output, input_a)?;
                let output_gpr = self.load_slice_gpr(output, SliceType::Output);
                let temp_gpr = self.get_temp_gpr();
                self.emit_load_const_to_gpr(temp_gpr, input_b)?;

                assemble!(self.block.assembler();
                    ; imul Rq(output_gpr as u8), Rq(temp_gpr as u8)
                );
            }
            _ => {
                todo!("Currently unsupported size for multiply: {}", input_b.size())
            }
        }
        Ok(())
    }

    #[inline]
    fn emit_multiply_slice_slice(
        &mut self,
        output: JIRSlice,
        input_a: JIRSlice,
        input_b: JIRSlice,
    ) -> Result<(), JitError> {
        let (target_gp, operand_gp) = self.load_commutative_inputs(output, input_a, input_b)?;
        assemble!(self.block.assembler();
            ; imul Rq(target_gp as u8), Rq(operand_gp as u8)
        );
        Ok(())
    }
}
