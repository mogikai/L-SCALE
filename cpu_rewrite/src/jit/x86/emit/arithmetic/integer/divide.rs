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
    pub fn emit_divide(
        &mut self,
        output: JIRSlice,
        dividend: JIRValue,
        divisor: JIRValue,
    ) -> JITResult {
        assert_eq!(dividend.size(), divisor.size());
        assert_eq!(dividend.size(), output.size());
        match (dividend, divisor) {
            (JIRValue::Const(dividend), JIRValue::Const(divisor)) => {
                self.emit_copy(output, JIRConst::new(dividend.value().wrapping_div(divisor.value()), dividend.size()))?;
            }
            (JIRValue::Slice(dividend), JIRValue::Slice(divisor)) => {
                self.emit_divide_slice_slice(output, dividend, divisor)?;
            }
            (_, _) => {
                todo!()
            }
        }
        Ok(())
    }

    fn emit_divide_slice_slice(
        &mut self,
        output: JIRSlice,
        dividend: JIRSlice,
        divisor: JIRSlice,
    ) -> JITResult {
        // Div expects the dividend in D:A (or just AX for word/byte size)
        // Since we only deal with 64-bit numbers currently, it is enough to load A and zero D
        let dividend_lower_gpr = GPRegister::A;
        let dividend_upper_gpr = GPRegister::D;
        self.emit_load_slice_to_gpr(dividend_lower_gpr, dividend)?;
        // Make sure we have the D register and potentially flush existing contents to memory.
        self.temp_alloc(dividend_upper_gpr)?;
        self.emit_load_const_to_gpr(dividend_upper_gpr, JIRConst::new(0, dividend.size()))?;
        let divisor_gpr = self.load_slice_gpr(divisor);
        let output_gpr = self.get_slice_gp(output);

        assemble!(self.block_mut()?.assembler;
            ; div Rq(divisor_gpr as u8)
            // dividend_lower contains the quotient
            // dividend_upper contains the remainder, which we do not need here.
            ; mov Rq(output_gpr as u8), Rq(dividend_lower_gpr as u8)
        );
        self.temp_free(dividend_upper_gpr)?;
        Ok(())
    }
}
