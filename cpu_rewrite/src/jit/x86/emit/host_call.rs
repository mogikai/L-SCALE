use crate::jit::api::{
    JITResult,
    MogiJit,
};
use crate::jit::x86::{
    X86MogiJit,
    assemble,
};
use mogi_sleigh::spec::CustomOpId;
use mogi_sleigh::spec::jir::values::{
    JIRSlice,
    JIRValue,
};

impl X86MogiJit {
    pub fn emit_custom_op_call(
        &mut self,
        op_id: CustomOpId,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
        extra_args: &[JIRValue],
    ) -> JITResult {
        // TODO: Implement hooking mechanism that allows writing hooks for a custom op compiler
        //       that directly implements the custom ops in JIR
        // Make sure all slices are flushed, such that they cannot be trashed and are available to the impl callback.
        // TODO: Use a different function to flush for callbacks.
        //       We don't really need to flush rbx, rsp, rbp, r12, r13, r14, and r15
        //       since these are preserved by the caller.
        //       We also need to flush the temporary slice such that the custom op implementation
        //       has access to the data.
        self.emit_flush_slices()?;
        let Some(op) = &self.global_context.custom_op_impls[op_id] else {
            todo!("Return error: 'Custom operation has no implementation'")
        };
        let trampoline = op.trampoline;
        let op_ptr = op.callback;
        // sysv64 calling convention
        assemble!(self.block_mut()?.assembler;
            ; mov rax, QWORD trampoline as _
            // First argument of the trampoline: The cpu context
            ; mov rdi, cpu_context
            // Second argument of the trampoline: The callback pointer.
            ; mov rsi, QWORD op_ptr as _
            // Save the current stack pointer. RBX is preserved by the caller.
            ; mov rbx, rsp
            // Important: Make sure the stack is aligned to 16 bytes according to System V ABI before calling
            ; and rsp, DWORD -16
            // Call the trampoline
            ; call rax
            // restore the stack pointer
            ; mov rsp, rbx
        );
        Ok(())
    }
}
