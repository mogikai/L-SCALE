use crate::jit::api::JITResult;
use crate::jit::x86::regalloc::SliceType;
use crate::jit::x86::{
    X86MogiJit,
    assemble,
};
use mogi_sleigh::spec::jir::values::{
    JIRConst,
    JIRSlice,
    JIRValue,
};

impl<PageDataT> X86MogiJit<PageDataT> {
    /// Emit x86 assembly to perform an integer negation on the provided [JIRValue].
    /// The result is stored inside the output [JIRSlice].
    /// Providing a constant input will most likely result in a move of the already negated value.
    pub fn emit_negate(
        &mut self,
        output: JIRSlice,
        input: JIRValue,
    ) -> JITResult {
        match input {
            JIRValue::Const(value) => {
                self.emit_copy(output, JIRConst::new((value.value() as i128).wrapping_neg() as u128, value.size()))?
            }
            JIRValue::Slice(slice) => {
                // Copy into output, since neg uses the same register for input and output.
                // If output==input this is a no-op.
                self.emit_copy(output, input)?;
                let output_gpr = self.load_slice_gpr(output, SliceType::Output);
                assemble!(self.block_mut()?.assembler();
                    ; neg Rq(output_gpr as u8)
                );
            }
        }
        Ok(())
    }
}
