use crate::jit::api::{
    CustomOpArgs,
    CustomOpImpl,
    JITResult,
    JITTaskRuntimeContext,
    RuntimeError,
};
use crate::jit::x86::x64emitter::X64Emitter;
use crate::jit::x86::{
    X86GlobalContext,
    assemble,
};
use mogi_sleigh::spec::CustomOpId;
use mogi_sleigh::spec::jir::values::{
    JIRSlice,
    JIRValue,
};
use std::marker::PhantomData;

impl X64Emitter<'_> {
    pub fn emit_custom_op_call(
        &mut self,
        op_id: CustomOpId,
        output: Option<JIRSlice>,
        input_a: Option<JIRValue>,
        input_b: Option<JIRValue>,
        extra_args: &[JIRValue],
    ) -> JITResult {
        // Make sure all slices are flushed, such that they cannot be clobbered and are available to the impl callback.
        // TODO: Use a different function to flush for callbacks.
        //       We don't really need to flush rbx, rsp, rbp, r12, r13, r14, and r15
        //       since these are preserved by the callee.
        //       We also need to flush the temporary slice such that the custom op implementation
        //       has access to the data.
        self.save_loaded_slices()?;
        let Some(op) = &self.global_context.custom_op_impls[op_id] else {
            todo!("Return error: 'Custom operation has no implementation'")
        };
        let trampoline = op.trampoline;
        let op_ptr = op.callback;

        let args = Box::new(CustomOpArgs {
            output,
            input_a,
            input_b,
            extra_args: extra_args.to_vec(),
        });
        let args_ptr = args.as_ref() as *const _;
        self.block.add_custom_op_args(args);
        // sysv64 calling convention
        assemble!(self.block.assembler();
            // Move the trampoline to rax. We later call rax.
            ; mov rax, QWORD trampoline as _
            // First argument of the trampoline: The cpu context
            ; rdgsbase rdi
            // Second argument of the trampoline: The callback pointer.
            ; mov rsi, QWORD op_ptr as _
            // Third argument of the trampoline: The args pointer.
            ; mov rdx, QWORD args_ptr as _
            // Save the current stack pointer. RBX is preserved by the caller.
            ; mov rbx, rsp
            // Important: Make sure the stack is aligned to 16 bytes according to System V ABI before calling
            ; and rsp, DWORD -16
            // Call the trampoline
            ; call rax
            // restore the stack pointer
            ; mov rsp, rbx
        );
        self.reload_loaded_slices()?;
        Ok(())
    }
}

type CustomOpTrampoline<ContextT> = unsafe extern "sysv64" fn(&mut ContextT, *mut (), &CustomOpArgs);

/// The custom op callback consists of a trampoline function that acts as a transition layer between assembly
/// and rust code. Usually, the rust compiler will be able to inline the closures into this trampoline.
/// The callback signature is defined in [CustomOpImpl]. The callback gets the [JITTaskRuntimeContext] and the [CustomOpArgs].
pub struct CustomOpCallback<ContextT: JITTaskRuntimeContext> {
    trampoline: CustomOpTrampoline<ContextT>,
    callback: *mut (),
    phantom_data: PhantomData<ContextT>,
}

impl<ContextT> CustomOpCallback<ContextT>
where
    ContextT: JITTaskRuntimeContext,
{
    pub fn new<CB>(callback: CB) -> Self
    where
        CB: CustomOpImpl<ContextT>,
    {
        Self {
            trampoline: trampoline::<CB, ContextT>,
            callback: Box::into_raw(Box::new(callback)) as *mut (),
            phantom_data: Default::default(),
        }
    }
}

#[inline]
/// SAFETY: The trampoline is called by the jit runtime. The runtime must ensure that `callback` points to a valid
///         closure.
unsafe extern "sysv64" fn trampoline<CB, ContextT>(
    context: &mut ContextT,
    callback: *mut (),
    args: &CustomOpArgs,
) where
    CB: CustomOpImpl<ContextT>,
    ContextT: JITTaskRuntimeContext,
{
    let callback = callback as *mut CB;
    // TODO: Instead of unwrapping, we want to force a jit exit upon returning and pass the error on to the
    //       caller of the jit.
    unsafe { (*callback)(context, args).unwrap() }
}
