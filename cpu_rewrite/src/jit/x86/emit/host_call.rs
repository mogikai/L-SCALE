use crate::jit::api::{
    JITResult,
    MogiJit,
};
use crate::jit::x86::{
    Callback,
    CustomOpArgs,
    Trampoline,
    X86GlobalContext,
    X86MogiJit,
    X86TaskContext,
    assemble,
};
use mogi_sleigh::spec::CustomOpId;
use mogi_sleigh::spec::jir::values::{
    JIRSlice,
    JIRValue,
};
use mogi_utilities::index_map::MapKey;
use std::ffi::c_void;
use std::pin::Pin;
use tracing::debug;

impl X86MogiJit {
    pub fn emit_custom_op_call(
        &mut self,
        op_id: CustomOpId,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
        extra_args: &[JIRValue],
    ) -> JITResult {
        // Make sure all slices are flushed, such that they cannot be trashed.
        // TODO: Use a different function to flush for callbacks.
        //       We don't really need to flush rbx, rsp, rbp, r12, r13, r14, and r15
        //       since these are preserved by the caller.
        self.emit_flush_slices()?;
        let callback = custom_op_dispatch;
        let Some(op) = &self.global_context.custom_op_impls[op_id] else {
            todo!("Return error: 'Custom operation has no implementation'")
        };
        let trampoline = op.trampoline;
        let op_ptr = op.callback;
        assemble!(self.block_mut()?.assembler;
            ; mov rax, QWORD callback as _
            ; mov rdi, cpu_context
            ; mov rsi, QWORD trampoline as _
            ; mov rdx, QWORD op_ptr as _
            // Save the current stack pointer. RBX is preserved by the caller.
            ; mov rbx, rsp
            // Important: Make sure the stack is aligned to 16 bytes according to System V ABI before calling
            ; and rsp, DWORD -16
            ; call rax
            // restore the stack pointer
            ; mov rsp, rbx
        );
        Ok(())
    }
}

extern "C" fn custom_op_dispatch(
    context: &mut X86TaskContext,
    trampoline: Trampoline<X86TaskContext>,
    callback: *mut (),
) {
    trampoline(context, callback);
}
