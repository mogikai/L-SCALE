use crate::jit::api::{
    CustomOpArgs,
    CustomOpFastImpl,
    CustomOpFastImplBinary,
    CustomOpFastImplUnary,
    CustomOpImpl,
    JITResult,
    JITTaskRuntimeContext,
    RuntimeError,
};
use crate::jit::host::register::{
    HostReg,
    HostRegSet,
    RegClass,
};
use crate::jit::regalloc::requirements::{
    OperandConstraint,
    OperandKind,
    OperandRequest,
};
use crate::jit::x86::alloc_requirement::AllocRequirement;
use crate::jit::x86::registers::general_purpose::GPRegister;
use crate::jit::x86::task_context::X86TaskContext;
use crate::jit::x86::x64emitter::{
    X64AllocRequirements,
    X64Emitter,
};
use crate::jit::x86::{
    CustomOpImplKind,
    assemble,
};
use dynasmrt::DynasmApi;
use mogi_sleigh::spec::CustomOpId;
use mogi_sleigh::spec::jir::values::{
    JIRSlice,
    JIRValue,
};
use std::ffi::c_void;
use std::marker::PhantomData;
use std::slice;
use zerocopy::IntoBytes;

// rax, rdi, rsi, rdx, rcx, r8, r9, r10, r11 are clobbered by function calls.
const SYSV_CLOBBERS: HostRegSet = HostRegSet::empty()
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

impl X64AllocRequirements {
    pub fn custom_op_call_requirements(
        requirement: &mut AllocRequirement,
        op_kind: CustomOpImplKind,
        output: Option<JIRSlice>,
        input_a: Option<JIRValue>,
        input_b: Option<JIRValue>,
        extra_args: &[JIRValue],
    ) -> Result<(), String> {
        match op_kind {
            CustomOpImplKind::Unimpl => Err("Unimplemented custom operation".to_string())?,
            CustomOpImplKind::BlockEnding => {
                // Block ending custom ops do not have any allocator requirements since they end the block
                // and thus flush everything to memory.
            }
            CustomOpImplKind::FastImpl(fast_impl) => match fast_impl {
                CustomOpFastImpl::Binary(binary) => {
                    Self::binary_fast_impl_requirements(requirement, binary, output, input_a, input_b, extra_args)
                }
                CustomOpFastImpl::Unary(unary) => {
                    Self::unary_fast_impl_requirements(requirement, unary, output, input_a, extra_args)
                }
            },
            CustomOpImplKind::Callback(_) => {}
        }
        Ok(())
    }

    fn binary_fast_impl_requirements(
        requirement: &mut AllocRequirement,
        fast_impl: CustomOpFastImplBinary,
        output: Option<JIRSlice>,
        input_a: Option<JIRValue>,
        input_b: Option<JIRValue>,
        extra_args: &[JIRValue],
    ) {
        let Some(output) = output else {
            todo!("Emit error: fast impl for custom op requires an output slice. Error in sleigh specification?")
        };
        let Some(input_a) = input_a else {
            todo!("Emit error: fast impl for unary custom op requires an output slice. Error in sleigh specification?")
        };
        let Some(input_b) = input_b else {
            todo!("Emit error: fast impl for unary custom op requires an output slice. Error in sleigh specification?")
        };
        // Check if we need to load the args into registers or if we need to require that they are flushed to memory.
        let needs_reg_args = match output.size() {
            1 => fast_impl.impl_8.is_some(),
            2 => fast_impl.impl_16.is_some(),
            4 => fast_impl.impl_32.is_some(),
            8 => fast_impl.impl_64.is_some(),
            16 => fast_impl.impl_128.is_some(),
            _ => false,
        };
        if needs_reg_args {
            // Reserve (AX, DX) as output, since sysv64 returns in RAX or RAX, RDX if 128 bit.
            Self::require_large_int_write_fixed(
                requirement,
                output,
                GPRegister::AX.into_host_reg(),
                GPRegister::DX.into_host_reg(),
            );
            // Either load DI with the slice or mark DI as clobber, since we need to load the const to DI.
            match input_a {
                JIRValue::Slice(input_a) => {
                    Self::request_fast_impl_large_parameter(
                        requirement,
                        GPRegister::DI,
                        GPRegister::SI,
                        OperandKind::ReadReload,
                        input_a,
                    );
                }
                JIRValue::Const(_) => {
                    // Remove ax from clobbers since it is the output
                    *requirement.clobbers_mut() = SYSV_CLOBBERS.without(GPRegister::AX.into_host_reg());
                    if output.size() == 16 {
                        // Also remove DX if we are dealing with 128 bit values.
                        requirement
                            .clobbers_mut()
                            .remove(GPRegister::DX.into_host_reg());
                    }
                }
            }
            match input_b {
                JIRValue::Slice(input_b) => {
                    Self::request_fast_impl_large_parameter(
                        requirement,
                        if input_a.size() <= 8 {
                            GPRegister::SI
                        } else {
                            GPRegister::DX
                        },
                        GPRegister::CX,
                        OperandKind::ReadReload,
                        input_b,
                    );
                }
                JIRValue::Const(_) => {
                    // Remove ax from clobbers since it is the output
                    *requirement.clobbers_mut() = SYSV_CLOBBERS.without(GPRegister::AX.into_host_reg());
                    if output.size() == 16 {
                        // Also remove DX if we are dealing with 128 bit values.
                        requirement
                            .clobbers_mut()
                            .remove(GPRegister::DX.into_host_reg());
                    }
                }
            }
        } else {
            // Request output and input as memory arguments, ensuring they are written/read from memory.
            requirement.add_request(OperandRequest::int_write_mem(output));
            input_a.is_slice_then(|input| requirement.add_request(OperandRequest::int_read_mem(*input)));
            input_b.is_slice_then(|input| requirement.add_request(OperandRequest::int_read_mem(*input)));
            // Clobber everything.
            *requirement.clobbers_mut() = SYSV_CLOBBERS;
        }
    }

    fn unary_fast_impl_requirements(
        requirement: &mut AllocRequirement,
        fast_impl: CustomOpFastImplUnary,
        output: Option<JIRSlice>,
        input: Option<JIRValue>,
        extra_args: &[JIRValue],
    ) {
        let Some(output) = output else {
            todo!("Emit error: fast impl for custom op requires an output slice. Error in sleigh specification?")
        };
        let Some(input) = input else {
            todo!("Emit error: fast impl for unary custom op requires an output slice. Error in sleigh specification?")
        };
        // Check if we need to load the args into registers or if we need to require that they are flushed to memory.
        let needs_reg_args = match input.size() {
            1 => fast_impl.impl_8.is_some(),
            2 => fast_impl.impl_16.is_some(),
            4 => fast_impl.impl_32.is_some(),
            8 => fast_impl.impl_64.is_some(),
            16 => fast_impl.impl_128.is_some(),
            _ => false,
        };
        if needs_reg_args {
            // Reserve (AX, DX) as output, since sysv64 returns in RAX or RAX, RDX if 128 bit.
            Self::require_large_int_write_fixed(
                requirement,
                output,
                GPRegister::AX.into_host_reg(),
                GPRegister::DX.into_host_reg(),
            );
            // Either load DI with the slice or mark DI as clobber, since we need to load the const to DI.
            match input {
                JIRValue::Slice(input_a) => {
                    Self::request_fast_impl_large_parameter(
                        requirement,
                        GPRegister::DI,
                        GPRegister::SI,
                        OperandKind::ReadReload,
                        input_a,
                    );
                }
                JIRValue::Const(input_a) => {
                    // Remove ax from clobbers since it is the output
                    *requirement.clobbers_mut() = SYSV_CLOBBERS.without(GPRegister::AX.into_host_reg());
                    if output.size() == 16 {
                        // Also remove DX if we are dealing with 128 bit values.
                        requirement
                            .clobbers_mut()
                            .remove(GPRegister::DX.into_host_reg());
                    }
                }
            }
        } else {
            // Request output and input as memory arguments, ensuring they are written/read from memory.
            requirement.add_request(OperandRequest::int_write_mem(output));
            input.is_slice_then(|input| requirement.add_request(OperandRequest::int_read_mem(*input)));
            // Clobber everything.
            *requirement.clobbers_mut() = SYSV_CLOBBERS;
        }
    }

    fn request_fast_impl_large_parameter(
        requirement: &mut AllocRequirement,
        lower_reg: GPRegister,
        upper_reg: GPRegister,
        kind: OperandKind,
        input: JIRSlice,
    ) {
        match input.size() {
            size if size <= 8 => {
                requirement.add_request(OperandRequest {
                    slice: input,
                    constraint: OperandConstraint::FixedReg(lower_reg.into_host_reg()),
                    kind,
                    class: RegClass::Int,
                });
                // Remove ax and lower from clobbers since ax is the output and lower is the input
                *requirement.clobbers_mut() = SYSV_CLOBBERS
                    .without(GPRegister::AX.into_host_reg())
                    .without(lower_reg.into_host_reg());
            }
            size if size == 16 => {
                let (lower, upper) = input.split(8);
                requirement.add_request(OperandRequest {
                    slice: lower,
                    constraint: OperandConstraint::FixedReg(lower_reg.into_host_reg()),
                    kind,
                    class: RegClass::Int,
                });
                requirement.add_request(OperandRequest {
                    slice: upper,
                    constraint: OperandConstraint::FixedReg(upper_reg.into_host_reg()),
                    kind,
                    class: RegClass::Int,
                });
                // Remove ax, dx, lower, and upper from clobbers since (ax, dx) is the output and (lower, upper) is the input
                *requirement.clobbers_mut() = SYSV_CLOBBERS
                    .without(GPRegister::AX.into_host_reg())
                    .without(GPRegister::DX.into_host_reg())
                    .without(lower_reg.into_host_reg())
                    .without(upper_reg.into_host_reg());
            }
            _ => panic!("Unsupported slice size"),
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
            CustomOpImplKind::FastImpl(fast_impl) => match fast_impl {
                CustomOpFastImpl::Binary(binary) => self.emit_binary_fast_impl(
                    binary,
                    output.expect("This is guaranteed by the regalloc phase"),
                    input_a.expect("This is guaranteed by the regalloc phase"),
                    input_b.expect("This is guaranteed by the regalloc phase"),
                ),
                CustomOpFastImpl::Unary(unary) => self.emit_unary_fast_impl(
                    unary,
                    output.expect("This is guaranteed by the regalloc phase"),
                    input_a.expect("This is guaranteed by the regalloc phase"),
                ),
            },
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

    fn emit_binary_fast_impl(
        &mut self,
        binary: CustomOpFastImplBinary,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        let fn_impl = match output.size() {
            1 => binary.impl_8.map(|f| f as *const c_void),
            2 => binary.impl_16.map(|f| f as *const c_void),
            4 => binary.impl_32.map(|f| f as *const c_void),
            8 => binary.impl_64.map(|f| f as *const c_void),
            16 => binary.impl_128.map(|f| f as *const c_void),
            _ => None,
        };
        assemble!(self.block.assembler();
            // Save the current stack pointer. RBX is preserved by the caller.
            ; mov rbx, rsp
        );
        let fn_impl = if let Some(fn_impl) = fn_impl {
            assert_eq!(input_a.size(), input_b.size());
            // The register allocator made sure that RDI is already loaded with
            // the correct value if we are dealing with a slice.
            // If we are dealing with a const, we need to load it into RDI (, RSI).
            if let JIRValue::Const(input_a) = input_a {
                if input_a.size() == 16 {
                    let (lower, upper) = input_a.split(8);
                    self.load_const(GPRegister::DI.into_host_reg(), lower)?;
                    self.load_const(GPRegister::SI.into_host_reg(), upper)?;
                } else {
                    self.load_const(GPRegister::DI.into_host_reg(), input_a)?;
                }
            }
            if let JIRValue::Const(input_b) = input_b {
                if input_b.size() == 16 {
                    let (lower, upper) = input_b.split(8);
                    self.load_const(GPRegister::DX.into_host_reg(), lower)?;
                    self.load_const(GPRegister::CX.into_host_reg(), upper)?;
                } else {
                    self.load_const(GPRegister::SI.into_host_reg(), input_b)?;
                }
            }
            fn_impl
        } else {
            // No specialized function available -> use generic slice variant.
            if output == input_a || output == input_b {
                // output pointer is in RDI
                self.load_slice_pointer(GPRegister::DI, output);
                // output length is in RSI
                self.load_const(GPRegister::SI, output.size())?;
                self.load_input(GPRegister::DX, GPRegister::CX, if output == input_a { input_b } else { input_a })?;
                binary.impl_slice_in_out as *const c_void
            } else if input_a.map_slice_or_default(true, |s| !s.overlaps(&output))
                && input_b.map_slice_or_default(true, |s| !s.overlaps(&output))
            {
                // neither input overlaps with the output
                // output pointer is in RDI
                self.load_slice_pointer(GPRegister::DI, output);
                // output length is in RSI
                self.load_const(GPRegister::SI, output.size())?;
                self.load_input(GPRegister::DX, GPRegister::CX, input_a)?;
                self.load_input(GPRegister::R8, GPRegister::R9, input_a)?;
                binary.impl_slice_non_overlapping as *const c_void
            } else {
                todo!()
            }
        };

        // TODO: Either check how we can maybe inline these function calls, or maybe avoid loading into rax
        //       if the target is located somewhere that can be described as a 32 bit offset?
        assemble!(self.block.assembler();
            // Important: Make sure the stack is aligned to 16 bytes according to System V ABI before calling
            ; and rsp, DWORD -16
            // Load the callback address into RAX
            ; mov rax, QWORD fn_impl as _
            // Perform the call
            ; call rax
            // restore the stack pointer
            ; mov rsp, rbx
        );
        Ok(())
    }

    fn emit_unary_fast_impl(
        &mut self,
        unary: CustomOpFastImplUnary,
        output: JIRSlice,
        input: JIRValue,
    ) -> JITResult {
        let fn_impl = match input.size() {
            1 => unary.impl_8.map(|f| f as *const c_void),
            2 => unary.impl_16.map(|f| f as *const c_void),
            4 => unary.impl_32.map(|f| f as *const c_void),
            8 => unary.impl_64.map(|f| f as *const c_void),
            16 => unary.impl_128.map(|f| f as *const c_void),
            _ => None,
        };
        assemble!(self.block.assembler();
            // Save the current stack pointer. RBX is preserved by the caller.
            ; mov rbx, rsp
        );
        let fn_impl = if let Some(fn_impl) = fn_impl {
            // The register allocator made sure that RDI is already loaded with
            // the correct value if we are dealing with a slice.
            // If we are dealing with a const, we need to load it into RDI (, RSI).
            if let JIRValue::Const(input_a) = input {
                if input_a.size() == 16 {
                    let (lower, upper) = input_a.split(8);
                    self.load_const(GPRegister::DI.into_host_reg(), lower)?;
                    self.load_const(GPRegister::SI.into_host_reg(), upper)?;
                } else {
                    self.load_const(GPRegister::DI.into_host_reg(), input_a)?;
                }
            }
            fn_impl
        } else {
            // No specialized function available -> use generic slice variant.
            if output == input {
                self.load_slice_pointer(GPRegister::DI, output);
                self.load_const(GPRegister::SI, output.size())?;
                unary.impl_slice_in_out as *const c_void
            } else if input.map_slice_or_default(true, |s| !s.overlaps(&output)) {
                self.load_slice_pointer(GPRegister::DI, output);
                self.load_const(GPRegister::SI, output.size())?;
                self.load_input(GPRegister::DX, GPRegister::CX, input)?;
                unary.impl_slice_non_overlapping as *const c_void
            } else {
                // // output pointer is in RDI
                // self.load_slice_pointer(GPRegister::DI, output);
                // // output length is in RSI
                // self.load_const(GPRegister::SI, output.size())?;
                // self.load_input(GPRegister::DX, GPRegister::CX, input)?;
                // unary.impl_slice_non_overlapping as *const c_void;
                todo!("Implement overlapping unary fast impl")
            }
        };

        // TODO: Either check how we can maybe inline these function calls, or maybe avoid loading into rax
        //       if the target is located somewhere that can be described as a 32 bit offset?
        assemble!(self.block.assembler();
            // Important: Make sure the stack is aligned to 16 bytes according to System V ABI before calling
            ; and rsp, DWORD -16
            // Load the callback address into RAX
            ; mov rax, QWORD fn_impl as _
            // Perform the call
            ; call rax
            // restore the stack pointer
            ; mov rsp, rbx
        );
        Ok(())
    }

    fn load_slice_pointer(
        &mut self,
        reg: impl Into<HostReg>,
        slice: JIRSlice,
    ) {
        let reg = reg.into();
        assemble!(self.block.assembler();
            ; rdgsbase Rq(reg.hw_enc())
            ; lea Rq(reg.hw_enc()), [Rq(reg.hw_enc()) + i32::try_from(X86TaskContext::slice_offset(slice)).expect("Slice offset must fit into i32")]
        );
    }

    fn load_input(
        &mut self,
        ptr_reg: impl Into<HostReg>,
        len_reg: impl Into<HostReg>,
        input: JIRValue,
    ) -> JITResult {
        let ptr_reg = ptr_reg.into();
        let len_reg = len_reg.into();
        match input {
            JIRValue::Slice(input) => {
                self.load_slice_pointer(ptr_reg, input);
                self.load_const(len_reg, input.size())?;
            }
            JIRValue::Const(input) => {
                let (lower, upper) = input.split(8);
                assemble!(self.block.assembler();
                    // Allocate enough space on stack for input. Consts are at most 16 bytes large
                    ; sub rsp, 16
                    // Load the lower part. Need to use register as intermediate since we can only move 64 bits to reg.
                    ; mov Rq(ptr_reg.hw_enc()), QWORD lower.value() as _
                    ; mov QWORD [rsp], Rq(ptr_reg.hw_enc())
                    // Load the upper part. Need to use register as intermediate since we can only move 64 bits to reg.
                    ; mov Rq(ptr_reg.hw_enc()), QWORD upper.value() as _
                    ; mov QWORD [rsp + 8], Rq(ptr_reg.hw_enc())
                    // Move the pointer into the register.
                    ; mov Rq(ptr_reg.hw_enc()), rsp
                );
                self.load_const(len_reg, input.size())?;
            }
        }
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
