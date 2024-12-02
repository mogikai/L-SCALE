use crate::jit::api::JitError;
use crate::jit::x86::X86MogiJit;
use mogi_sleigh::spec::jir::values::{
    JIRSlice,
    JIRValue,
};

impl X86MogiJit {
    fn emit_bitwise_and(
        &mut self,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> Result<(), JitError> {
        // TODO: It might make sense to directly put the memory reference into the instruction, instead of loading
        //       to a register first. However, this requires that we know if the value is used in subsequent instructions
        //       because if it is used, we have to load it in a register. The exception to this is for cases like
        //       out = out OP b, where out does not have to be loaded first and we can just use the mem reference directly.
        let output_gp = self.load_slice_gp(output);
        Ok(())
    }
}
