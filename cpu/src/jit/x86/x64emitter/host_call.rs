use crate::jit::api::{
    CustomOpArgs,
    CustomOpImpl,
    JITResult,
    JITTaskRuntimeContext,
    RuntimeError,
};
use crate::jit::host::register::{
    HostRegSet,
    RegClass,
};
use crate::jit::regalloc::requirements::OperandRequest;
use crate::jit::x86::alloc_requirement::AllocRequirement;
use crate::jit::x86::registers::general_purpose::GPRegister;
use crate::jit::x86::task_context::X86TaskContext;
use crate::jit::x86::x64emitter::{
    X64AllocRequirements,
    X64Emitter,
};
use crate::jit::x86::{
    assemble,
    CustomOpImplKind,
};
use mogi_sleigh::spec::jir::values::{
    JIRSlice,
    JIRValue,
};
use mogi_sleigh::spec::CustomOpId;
use std::marker::PhantomData;

impl X64AllocRequirements {
    pub fn custom_op_call_requirements(
        requirement: &mut AllocRequirement,
        op_kind: CustomOpImplKind,
        output: Option<JIRSlice>,
        input_a: Option<JIRValue>,
        input_b: Option<JIRValue>,
        extra_args: &[JIRValue],
    ) {
        // rax, rdi, rsi, rdx, rcx, r8, r9, r10, r11 are clobbered by function calls.
        let sysv_clobbers = HostRegSet::empty()
            .with(GPRegister::AX.into_host_reg())
            .with(GPRegister::DI.into_host_reg())
            .with(GPRegister::SI.into_host_reg())
            .with(GPRegister::DX.into_host_reg())
            .with(GPRegister::CX.into_host_reg())
            // RBX is also clobbered since we need to save the stack pointer.
            .with(GPRegister::BX.into_host_reg())
            .with(GPRegister::R8.into_host_reg())
            .with(GPRegister::R9.into_host_reg())
            .with(GPRegister::R10.into_host_reg())
            .with(GPRegister::R11.into_host_reg());

        match op_kind {
            CustomOpImplKind::Unimpl => {
                todo!("Emit error? Unimplemented custom op")
            }
            CustomOpImplKind::BlockEnding => {
                // Block ending custom ops do not have any allocator requirements since they end the block
                // and thus flush everything to memory.
            }
            CustomOpImplKind::BinaryFastImpl64(_) => {}
            CustomOpImplKind::UnaryFastImpl64(_) => {
                let Some(output) = output else {
                    todo!(
                        "Emit error: fast impl for custom op requires an output slice. Error in sleigh specification?"
                    )
                };
                let Some(input_a) = input_a else {
                    todo!(
                        "Emit error: fast impl for unary custom op requires an output slice. Error in sleigh specification?"
                    )
                };
                // Reserve AX as output, since sysv64 returns in RAX
                requirement.add_request(OperandRequest::int_write_fixed(GPRegister::AX.into_host_reg(), output));
                // Either load DI with the slice or mark DI as clobber, since we need to load the const to DI.
                match input_a {
                    JIRValue::Slice(input_a) => {
                        requirement.add_request(OperandRequest::int_read_fixed_reg_reload(
                            GPRegister::DI.into_host_reg(),
                            input_a,
                        ));
                        // Remove ax and di from clobbers since ax is the output and di is the input
                        *requirement.clobbers_mut() = sysv_clobbers
                            .without(GPRegister::AX.into_host_reg())
                            .without(GPRegister::DI.into_host_reg());
                    }
                    JIRValue::Const(input_a) => {
                        // Remove ax from clobbers since it is the output
                        *requirement.clobbers_mut() = sysv_clobbers.without(GPRegister::AX.into_host_reg());
                    }
                }
            }
            CustomOpImplKind::FastImplSlice(_) => {}
            CustomOpImplKind::Callback(_) => {}
        }
    }
}

impl X64Emitter<'_> {
    pub fn emit_custom_op_call(
        &mut self,
        op_id: CustomOpId,
        output: Option<JIRSlice>,
        input_a: Option<JIRValue>,
        input_b: Option<JIRValue>,
        extra_args: &[JIRValue],
    ) -> JITResult {
        match self.global_context.custom_op_impls[op_id] {
            CustomOpImplKind::BlockEnding => {
                let scratch = self.allocations.scratch_register(RegClass::Int);
                assemble!(self.block.assembler();
                    ; mov Rq(scratch.hw_enc()), QWORD u64::from(RuntimeError::CustomOp(op_id)) as _
                    ; gs mov QWORD [X86TaskContext::runtime_error_offset() as _], Rq(scratch.hw_enc())
                );
                Ok(())
            }
            CustomOpImplKind::Unimpl => {
                todo!("Unimplemented custom operation")
            }
            CustomOpImplKind::BinaryFastImpl64(_) => {
                todo!("implement fast impl 64")
            }
            CustomOpImplKind::UnaryFastImpl64(fn_impl) => self.emit_unary_fast_impl_64(
                fn_impl,
                output.expect("This was checked during regalloc phase"),
                input_a.expect("This was checked during regalloc phase"),
            ),
            CustomOpImplKind::FastImplSlice(_) => {
                todo!("implement fast impl slice")
            }
            CustomOpImplKind::Callback(callback) => {
                self.emit_custom_op_callback(callback, output, input_a, input_b, extra_args)
            }
        }
    }

    fn emit_custom_op_callback(
        &mut self,
        op_impl: CustomOpCallback<X86TaskContext>,
        output: Option<JIRSlice>,
        input_a: Option<JIRValue>,
        input_b: Option<JIRValue>,
        extra_args: &[JIRValue],
    ) -> JITResult {
        // Make sure all slices are flushed, such that they cannot be clobbered and are available to the impl callback.
        // TODO: Use a different function to flush for callbacks.
        //       We don't really need to flush rbx, rsp, rbp, r12, r13, r14, and r15
        //       since these are preserved by the callee.
        // TODO: Flush all slices used by the custom op and leave the others as is.
        self.save_loaded_slices()?;
        let trampoline = op_impl.trampoline;
        let op_ptr = op_impl.callback;

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

    fn emit_unary_fast_impl_64(
        &mut self,
        fn_impl: extern "sysv64" fn(u64) -> u64,
        output: JIRSlice,
        input_a: JIRValue,
    ) -> JITResult {
        // The register allocator made sure that RDI is already loaded with
        // the correct value if we are dealing with a slice.
        // If we are dealing with a const, we need to load it into RDI.
        if let JIRValue::Const(input_a) = input_a {
            self.load_const(GPRegister::DI.into_host_reg(), input_a)?;
        }
        assemble!(self.block.assembler();
            // Load the callback address into RAX
            ; mov rax, QWORD fn_impl as _
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

type CustomOpTrampoline<ContextT> = unsafe extern "sysv64" fn(&mut ContextT, *mut (), &CustomOpArgs);

/// The custom op callback consists of a trampoline function that acts as a transition layer between assembly
/// and rust code. Usually, the rust compiler will be able to inline the closures into this trampoline.
/// The callback signature is defined in [CustomOpImpl]. The callback gets the [JITTaskRuntimeContext] and the [CustomOpArgs].
#[derive(Debug)]
pub struct CustomOpCallback<ContextT: JITTaskRuntimeContext> {
    trampoline: CustomOpTrampoline<ContextT>,
    callback: *mut (),
    phantom_data: PhantomData<ContextT>,
}

impl Clone for CustomOpCallback<X86TaskContext> {
    fn clone(&self) -> Self {
        Self {
            trampoline: self.trampoline,
            callback: self.callback,
            phantom_data: Default::default(),
        }
    }
}

impl Copy for CustomOpCallback<X86TaskContext> {}

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
