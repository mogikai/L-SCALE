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
    /// Emit instructions to normalize the boolean inputs
    fn emit_normalize_boolean_inputs(
        &mut self,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        self.emit_normalize_boolean_input(input_a)?;
        self.emit_normalize_boolean_input(input_b)
    }

    fn emit_normalize_boolean_input(
        &mut self,
        input: JIRValue,
    ) -> JITResult {
        match input {
            JIRValue::Slice(slice) => {
                // TODO: We can emit this normalization in some cases, if we have a marker somewhere
                //       where we can save if a value has already been normalized.
                //       this is also the case for example if `input` is the result of a boolean expression.
                let input_gpr = self.load_slice_gpr(slice, SliceType::Input);
                assemble!(self.block_mut()?.assembler();
                    ; cmp Rb(input_gpr as u8), 0
                    ; setnz Rb(input_gpr as u8)
                );
            }
            JIRValue::Const(value) => {
                if value.value() > 1 {
                    todo!("Error: Boolean values may only be zero or one.")
                }
            }
        }
        Ok(())
    }

    pub fn emit_boolean_and(
        &mut self,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        if output.size() != 1 || output.size() != input_a.size() || input_a.size() != input_b.size() {
            todo!("Invalid boolean size")
        }
        self.emit_normalize_boolean_inputs(input_a, input_b)?;
        self.emit_bitwise_and(output, input_a, input_b)
    }
}
