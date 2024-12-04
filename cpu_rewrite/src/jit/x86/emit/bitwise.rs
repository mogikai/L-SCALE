use crate::jit::api::{
    JITResult,
    JitError,
};
use crate::jit::x86::{
    GPRegister,
    X86MogiJit,
    assemble,
};
use mogi_sleigh::spec::jir::values::{
    JIRConst,
    JIRSlice,
    JIRValue,
};

impl X86MogiJit {
    pub fn emit_bitwise_and(
        &mut self,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        assert_eq!(input_a.size(), input_b.size());
        // TODO: It might make sense to directly put the memory reference into the instruction, instead of loading
        //       to a register first. However, this requires that we know if the value is used in subsequent instructions
        //       because if it is used, we have to load it in a register. The exception to this is for cases like
        //       out = out OP b, where out does not have to be loaded first and we can just use the mem reference directly.
        match (input_a, input_b) {
            (JIRValue::Const(input_a), JIRValue::Const(input_b)) => {
                // In the const case we can simply calculate the value ahead of time and move it into the output.
                self.emit_copy(output, JIRConst::new(input_a.value() & input_b.value(), input_a.size()).into())?
            }
            (JIRValue::Slice(input_a), JIRValue::Slice(input_b)) => {
                let (target_gp, operand_gp) = self.load_commutative_inputs(output, input_a, input_b)?;
                assemble!(self.block_mut()?.assembler;
                    ; and Rq(target_gp as u8), Rq(operand_gp as u8)
                );
            }
            (_, _) => {}
        }
        Ok(())
    }
}
