use crate::jit::api::JITResult;
use crate::jit::x86::{
    GPRegister,
    X86MogiJit,
    assemble,
};
use bitvec::macros::internal::funty::Integral;
use mogi_sleigh::spec::jir::values::{
    JIRConst,
    JIRSlice,
    JIRValue,
};

impl X86MogiJit {
    /// Unsigned integer division.
    #[inline]
    pub fn emit_divide(
        &mut self,
        output: JIRSlice,
        dividend: JIRValue,
        divisor: JIRValue,
    ) -> JITResult {
        assert_eq!(dividend.size(), divisor.size());
        assert_eq!(dividend.size(), output.size());
        if let (JIRValue::Const(dividend), JIRValue::Const(divisor)) = (dividend, divisor) {
            // If both arguments are const we can emit a move of the calculated value.
            return self
                .emit_copy(output, JIRConst::new(dividend.value().wrapping_div(divisor.value()), dividend.size()));
        }
        self.emit_divide_value_value(output, dividend.into(), divisor)?;
        Ok(())
    }

    #[inline]
    fn emit_divide_value_value(
        &mut self,
        output: JIRSlice,
        dividend: JIRValue,
        divisor: JIRValue,
    ) -> JITResult {
        // Div expects the dividend in D:A (or just AX for word/byte size)
        // Since we only deal with 64-bit numbers currently, it is enough to load A and zero D
        let dividend_lower_gpr = GPRegister::A;
        let dividend_upper_gpr = GPRegister::D;
        self.emit_load_value_to_gpr(dividend_lower_gpr, dividend)?;
        // Make sure we have the D register and potentially flush existing contents to memory.
        self.reserve_gp_as_temp(dividend_upper_gpr)?;
        // Set the upper part of the dividend to zero, since we are only dealing with 64 bit values here.
        self.emit_load_const_to_gpr(dividend_upper_gpr, JIRConst::new(0, dividend.size()))?;

        // Fetch the divisor register and load the value.
        let divisor_gpr = match divisor {
            JIRValue::Slice(divisor_slice) => self.load_slice_gpr(divisor_slice),
            JIRValue::Const(divisor_const) => {
                let tmp = self.temp_alloc();
                self.emit_load_const_to_gpr(tmp, divisor_const)?;
                tmp
            }
        };

        // Fetch the output gpr and perform the division.
        let output_gpr = self.get_slice_gp(output);
        assemble!(self.block_mut()?.assembler;
            ; div Rq(divisor_gpr as u8)
        );
        // Finally we can copy over the result into the output if they are in different registers.
        if output_gpr != dividend_lower_gpr {
            assemble!(self.block_mut()?.assembler;
                // dividend_lower contains the quotient
                // dividend_upper contains the remainder, which we do not need here.
                ; mov Rq(output_gpr as u8), Rq(dividend_lower_gpr as u8)
            );
        }
        // And we cannot forget to free up the temporary.
        self.temp_free(dividend_upper_gpr)?;
        Ok(())
    }

    pub fn emit_signed_divide(
        &mut self,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        match (input_a, input_b) {
            (JIRValue::Const(dividend), JIRValue::Const(divisor)) => {
                let result = (dividend.value() as i128).wrapping_div(divisor.value() as i128) as u128;
                self.emit_copy(output, JIRConst::new(result, dividend.size()))
            }
            (_, _) => {
                todo!()
            }
        }
    }
}
