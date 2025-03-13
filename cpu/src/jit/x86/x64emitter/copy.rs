use crate::jit::api::JITResult;
use crate::jit::x86::emitter::OperandRequest;
use crate::jit::x86::x64emitter::regalloc::SliceType;
use crate::jit::x86::x64emitter::{
    X64AllocRequirements,
    X64Emitter,
};
use mogi_sleigh::spec::jir::values::{
    JIRSlice,
    JIRValue,
};

impl X64AllocRequirements {
    pub fn append_copy_operands(
        &mut self,
        target: JIRSlice,
        source: JIRValue,
    ) {
        self.operand_requests
            .push(OperandRequest::slice_write_any(target));
        if let JIRValue::Slice(source) = source {
            self.operand_requests
                .push(OperandRequest::slice_read_any(source));
        }
    }
}

impl X64Emitter<'_> {
    /// Emit instructions to copy the source [JIRValue] into the target [JIRSlice].
    #[inline]
    pub fn emit_copy(
        &mut self,
        target: JIRSlice,
        source: impl Into<JIRValue>,
    ) -> JITResult {
        let source = source.into();
        debug_assert_eq!(target.size(), source.size());

        let size = target.size();

        if target == source {
            // If target==source we just need to make sure the target is loaded.
            self.load_slice_gpr(target, SliceType::Output);
            return Ok(());
        }

        // Allocate and associate a GP register with the slice.
        // No load is needed, since we overwrite the data anyway for a copy.
        let target_reg = self.allocate_slice_gpr(target, SliceType::Output);
        self.emit_load_value_to_gpr(target_reg, source)
    }
}
