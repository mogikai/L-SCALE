use crate::jit::api::{
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
use crate::jit::x86::assemble;
use crate::jit::x86::registers::general_purpose::GPRegister;
use crate::jit::x86::task_context::X86TaskContext;
use crate::jit::x86::x64emitter::copy::CopyExtendMode;
use crate::jit::x86::x64emitter::helpers::reg;
use crate::jit::x86::x64emitter::{
    X64AllocRequirements,
    X64Emitter,
};
use mogi_sleigh::spec::jir::basic_block::InstId;
use mogi_sleigh::spec::jir::values::{
    JIRConst,
    JIRSlice,
    JIRValue,
};
use std::arch::asm;
use std::ffi::c_void;

enum DivisionType {
    Signed,
    Unsigned,
}

enum KeepDivisionResult {
    Quotient,
    Remainder,
}

impl X64AllocRequirements {
    pub fn divide_requirements(
        requirement: &mut AllocRequirement,
        output: JIRSlice,
        dividend: JIRValue,
        divisor: JIRValue,
    ) {
        Self::div_rem_requirements(requirement, output, dividend, divisor, KeepDivisionResult::Quotient);
    }

    fn div_rem_requirements(
        requirement: &mut AllocRequirement,
        output: JIRSlice,
        dividend: JIRValue,
        divisor: JIRValue,
        keep: KeepDivisionResult,
    ) {
        if dividend.size() != divisor.size() {
            todo!("Support division with differently sized arguments?")
        }
        if dividend.size() > 8 {
            return Self::div_rem_external_requirements(requirement, output, dividend, divisor);
        }
        match (dividend, divisor) {
            (JIRValue::Const(dividend), JIRValue::Const(divisor)) => {
                Self::copy_requirements(requirement, output, divide_const(dividend, divisor));
            }
            (JIRValue::Const(dividend), JIRValue::Slice(divisor)) => {
                // Dividend is loaded into RDX:RAX
                Self::load_output(requirement, output, keep);
                requirement.add_request(OperandRequest::int_read_any(divisor));
            }
            (JIRValue::Slice(dividend), JIRValue::Const(divisor)) => {
                Self::load_output(requirement, output, keep);
                requirement.add_request(OperandRequest::int_read_any(dividend));
            }
            (JIRValue::Slice(dividend), JIRValue::Slice(divisor)) => {
                Self::load_output(requirement, output, keep);
                requirement.add_request(OperandRequest::int_read_any(dividend));
                requirement.add_request(OperandRequest::int_read_any(divisor));
            }
        }
    }

    fn div_rem_external_requirements(
        requirement: &mut AllocRequirement,
        output: JIRSlice,
        dividend: JIRValue,
        divisor: JIRValue,
    ) {
        debug_assert_eq!(output.size(), 16);
        debug_assert_eq!(output.size(), dividend.size());
        debug_assert_eq!(output.size(), divisor.size());
        // rdi = dividend.low
        // rsi = dividend.high
        // rdx = divisor.low
        // rcx = divisor.high
        // r8 = cpu_context

        // rax, rdi, rsi, rdx, rcx, r8, r9, r10, r11 are clobbered by function calls.
        // rax, rdx are already used as output. rdi, rsi, rdx, r8 are used as inputs.
        let clobbers = HostRegSet::empty()
            // RBX is also clobbered since we need to save the stack pointer.
            .with(GPRegister::BX.into_host_reg())
            .with(GPRegister::R9.into_host_reg())
            .with(GPRegister::R10.into_host_reg())
            .with(GPRegister::R11.into_host_reg());

        let (output_lower, output_upper) = output.split(8);
        requirement.add_request(OperandRequest::int_write_fixed(GPRegister::AX.into_host_reg(), output_lower));
        requirement.add_request(OperandRequest::int_write_fixed(GPRegister::DX.into_host_reg(), output_upper));

        match (dividend, divisor) {
            (JIRValue::Const(dividend), JIRValue::Const(divisor)) => {
                unreachable!("Const Const division is handled elsewhere");
            }
            (JIRValue::Const(dividend), JIRValue::Slice(divisor)) => {
                let (divisor_lower, divisor_upper) = divisor.split(8);
                requirement.add_request(OperandRequest::int_read_fixed_reg_reload(
                    GPRegister::DX.into_host_reg(),
                    divisor_lower,
                ));
                requirement.add_request(OperandRequest::int_read_fixed_reg_reload(
                    GPRegister::CX.into_host_reg(),
                    divisor_upper,
                ));
                *requirement.clobbers_mut() = clobbers
                    .with(GPRegister::DI.into_host_reg())
                    .with(GPRegister::SI.into_host_reg());
            }
            (JIRValue::Slice(dividend), JIRValue::Const(divisor)) => {
                let (dividend_lower, dividend_upper) = dividend.split(8);
                requirement.add_request(OperandRequest::int_read_fixed_reg_reload(
                    GPRegister::DI.into_host_reg(),
                    dividend_lower,
                ));
                requirement.add_request(OperandRequest::int_read_fixed_reg_reload(
                    GPRegister::SI.into_host_reg(),
                    dividend_upper,
                ));
                *requirement.clobbers_mut() = clobbers
                    .with(GPRegister::DX.into_host_reg())
                    .with(GPRegister::CX.into_host_reg());
            }
            (JIRValue::Slice(dividend), JIRValue::Slice(divisor)) => {
                let (dividend_lower, dividend_upper) = dividend.split(8);
                requirement.add_request(OperandRequest::int_read_fixed_reg_reload(
                    GPRegister::DI.into_host_reg(),
                    dividend_lower,
                ));
                requirement.add_request(OperandRequest::int_read_fixed_reg_reload(
                    GPRegister::SI.into_host_reg(),
                    dividend_upper,
                ));
                let (divisor_lower, divisor_upper) = divisor.split(8);
                requirement.add_request(OperandRequest::int_read_fixed_reg_reload(
                    GPRegister::DX.into_host_reg(),
                    divisor_lower,
                ));
                requirement.add_request(OperandRequest::int_read_fixed_reg_reload(
                    GPRegister::CX.into_host_reg(),
                    divisor_upper,
                ));
            }
        }
    }

    fn load_output(
        requirement: &mut AllocRequirement,
        output: JIRSlice,
        keep: KeepDivisionResult,
    ) {
        match keep {
            KeepDivisionResult::Quotient => {
                // The quotient is always put into AX, and the remainder into DX
                requirement.add_request(OperandRequest::int_write_fixed(GPRegister::AX.into_host_reg(), output));
                *requirement.clobbers_mut() = HostRegSet::empty().with(GPRegister::DX.into_host_reg());
            }
            KeepDivisionResult::Remainder => {
                // The quotient is always put into AX, and the remainder into DX
                *requirement.clobbers_mut() = HostRegSet::empty().with(GPRegister::AX.into_host_reg());
                requirement.add_request(OperandRequest::int_write_fixed(GPRegister::DX.into_host_reg(), output));
            }
        }
    }

    pub fn divide_signed_requirements(
        requirement: &mut AllocRequirement,
        output: JIRSlice,
        dividend: JIRValue,
        divisor: JIRValue,
    ) {
        Self::divide_requirements(requirement, output, dividend, divisor)
    }

    pub fn remainder_requirements(
        requirement: &mut AllocRequirement,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) {
        Self::div_rem_requirements(requirement, output, input_a, input_b, KeepDivisionResult::Remainder);
    }

    pub fn signed_remainder_requirements(
        requirement: &mut AllocRequirement,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) {
        Self::div_rem_requirements(requirement, output, input_a, input_b, KeepDivisionResult::Remainder);
    }
}

impl X64Emitter<'_> {
    /// Unsigned integer division.
    #[inline]
    pub fn emit_divide(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        dividend: JIRValue,
        divisor: JIRValue,
    ) -> JITResult {
        self.emit_divide_internal(inst, output, dividend, divisor, DivisionType::Unsigned, KeepDivisionResult::Quotient)
    }

    fn emit_divide_internal(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        dividend: JIRValue,
        divisor: JIRValue,
        division_type: DivisionType,
        keep: KeepDivisionResult,
    ) -> JITResult {
        assert_eq!(dividend.size(), divisor.size());
        assert_eq!(dividend.size(), output.size());
        if let (JIRValue::Const(dividend), JIRValue::Const(divisor)) = (dividend, divisor) {
            // If both arguments are const we can emit a move of the calculated value.
            match (division_type, keep) {
                (DivisionType::Unsigned, KeepDivisionResult::Quotient) => {
                    self.emit_copy(inst, output, divide_const(dividend, divisor))
                }
                (DivisionType::Unsigned, KeepDivisionResult::Remainder) => {
                    self.emit_copy(inst, output, remainder_const(dividend, divisor))
                }
                (DivisionType::Signed, KeepDivisionResult::Quotient) => {
                    self.emit_copy(inst, output, divide_signed_const(dividend, divisor))
                }
                (DivisionType::Signed, KeepDivisionResult::Remainder) => {
                    self.emit_copy(inst, output, remainder_signed_const(dividend, divisor))
                }
            }
        } else {
            if output.size() > 8 {
                return self.emit_divide_value_value_128(inst, division_type, keep, output, dividend, divisor);
            }
            self.emit_divide_value_value(inst, division_type, output, dividend, divisor)
        }
    }

    #[inline]
    fn emit_divide_value_value(
        &mut self,
        inst: InstId,
        division_type: DivisionType,
        output: JIRSlice,
        dividend: JIRValue,
        divisor: JIRValue,
    ) -> JITResult {
        let output_reg = reg!(self.allocations, inst, 0);
        debug_assert!(
            output_reg == GPRegister::AX.into_host_reg() || output_reg == GPRegister::DX.into_host_reg(),
            "Expected the output to be in AX or DX, not in {output_reg}"
        );

        // Fetch the divisor register and load the value.
        let mut divisor_reg = match divisor {
            JIRValue::Slice(_) => {
                let divisor_reg = reg!(self.allocations, inst, if dividend.is_slice() { 2 } else { 1 });
                // We extend to rdx:rax, so the input must not be in DX.
                if divisor_reg == GPRegister::DX.into_host_reg() {
                    let scratch = self.allocations.scratch_register(RegClass::Int);
                    self.copy_reg(scratch, divisor_reg, divisor.size(), CopyExtendMode::None)?;
                    scratch
                } else {
                    divisor_reg
                }
            }
            JIRValue::Const(divisor_const) => {
                let tmp = self.allocations.scratch_register(RegClass::Int);
                self.load_const(tmp, divisor_const)?;
                tmp
            }
        };

        match dividend {
            JIRValue::Slice(_) => {
                // The dividend was also allocated, so the divisor is located in the third slot.
                let dividend_reg = reg!(self.allocations, inst, 1);
                // Copy over the dividend into RAX
                self.copy_reg(GPRegister::AX.into_host_reg(), dividend_reg, output.size(), CopyExtendMode::None)?;
            }
            JIRValue::Const(dividend) => {
                // The divisor might be in RAX, so we have to move it away first.
                if divisor_reg == GPRegister::AX.into_host_reg() {
                    let scratch = self.allocations.scratch_register(RegClass::Int);
                    self.copy_reg(scratch, divisor_reg, divisor.size(), CopyExtendMode::None)?;
                    divisor_reg = scratch;
                }
                self.load_const(GPRegister::AX.into_host_reg(), dividend)?;
            }
        };

        match division_type {
            DivisionType::Signed => {
                // Sign extend the divisor
                self.copy_reg(divisor_reg, divisor_reg, divisor.size(), CopyExtendMode::SignExtend(8))?;
                // First sign extend AX to 8 bytes.
                self.copy_reg(
                    GPRegister::AX.into_host_reg(),
                    GPRegister::AX.into_host_reg(),
                    dividend.size(),
                    CopyExtendMode::SignExtend(8),
                )?;
                // Sign extend rax to rdx:rax
                assemble!(self.block.assembler();
                    ; cqo
                );
            }
            DivisionType::Unsigned => {
                // Set the upper part of the dividend to zero, since we are only dealing with 64-bit values here.
                self.load_zero(GPRegister::DX.into_host_reg(), dividend.size())?;
            }
        }

        // Do the thing
        match division_type {
            DivisionType::Signed => {
                assemble!(self.block.assembler();
                    ; idiv Rq(divisor_reg.hw_enc())
                );
            }
            DivisionType::Unsigned => {
                assemble!(self.block.assembler();
                    ; div Rq(divisor_reg.hw_enc())
                );
            }
        }
        Ok(())
    }

    fn emit_divide_value_value_128(
        &mut self,
        inst: InstId,
        division_type: DivisionType,
        keep_division_result: KeepDivisionResult,
        output: JIRSlice,
        dividend: JIRValue,
        divisor: JIRValue,
    ) -> JITResult {
        let output_lower = reg!(self.allocations, inst, 0);
        let output_upper = reg!(self.allocations, inst, 1);
        debug_assert_eq!(output_lower, GPRegister::AX.into_host_reg());
        debug_assert_eq!(output_upper, GPRegister::DX.into_host_reg());

        match (dividend, divisor) {
            (JIRValue::Const(dividend), JIRValue::Const(divisor)) => {
                unreachable!("Const Const division is handled elsewhere");
            }
            (JIRValue::Const(dividend), JIRValue::Slice(divisor)) => {
                let (dividend_lower, dividend_upper) = dividend.split(8);
                let dividend_lower_reg = reg!(self.allocations, inst, 2);
                let dividend_upper_reg = reg!(self.allocations, inst, 3);
                self.load_const(dividend_lower_reg, dividend_lower)?;
                self.load_const(dividend_upper_reg, dividend_upper)?;
            }
            (JIRValue::Slice(dividend), JIRValue::Const(divisor)) => {
                let (divisor_lower, divisor_upper) = divisor.split(8);
                let divisor_lower_reg = reg!(self.allocations, inst, 2);
                let divisor_upper_reg = reg!(self.allocations, inst, 3);
                self.load_const(divisor_lower_reg, divisor_lower)?;
                self.load_const(divisor_upper_reg, divisor_upper)?;
            }
            (JIRValue::Slice(dividend), JIRValue::Slice(divisor)) => {
                let dividend_lower_reg = reg!(self.allocations, inst, 2);
                let dividend_upper_reg = reg!(self.allocations, inst, 3);
                let divisor_lower_reg = reg!(self.allocations, inst, 4);
                let divisor_upper_reg = reg!(self.allocations, inst, 5);
                debug_assert_eq!(dividend_lower_reg, GPRegister::DI.into_host_reg());
                debug_assert_eq!(dividend_upper_reg, GPRegister::SI.into_host_reg());
                debug_assert_eq!(divisor_lower_reg, GPRegister::DX.into_host_reg());
                debug_assert_eq!(divisor_upper_reg, GPRegister::CX.into_host_reg());
                // Arguments are already loaded correctly.
            }
        }

        let div_func = match (division_type, keep_division_result) {
            (DivisionType::Signed, KeepDivisionResult::Quotient) => sdiv128,
            (DivisionType::Unsigned, KeepDivisionResult::Quotient) => div128,
            (DivisionType::Signed, KeepDivisionResult::Remainder) => srem128,
            (DivisionType::Unsigned, KeepDivisionResult::Remainder) => rem128,
        };

        assemble!(self.block.assembler();
            // Load cpu context into r8
            ; rdgsbase r8
            // Save the current stack pointer. RBX is preserved by the caller.
            ; mov rbx, rsp
            // Important: Make sure the stack is aligned to 16 bytes according to System V ABI before calling
            ; and rsp, DWORD -16
            ; mov rax, QWORD div_func as _
            ; call rax
            // Restore the stack pointer
            ; mov rsp, rbx
            ; gs test [X86TaskContext::runtime_error_offset() as _], 0
            ; jnz >failure
            ; jmp >success
            ; failure:
        );
        self.save_loaded_slices()?;
        assemble!(self.block.assembler();
            ; jmp ->exit_failure
            ; success:
        );
        Ok(())
    }

    pub fn emit_signed_divide(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        dividend: JIRValue,
        divisor: JIRValue,
    ) -> JITResult {
        self.emit_divide_internal(inst, output, dividend, divisor, DivisionType::Signed, KeepDivisionResult::Quotient)
    }

    pub fn emit_remainder(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        dividend: JIRValue,
        divisor: JIRValue,
    ) -> JITResult {
        // We can delegate to this, since divide always calculates both quotient and remainder.
        // It is just a matter of which register the output is allocated to.
        self.emit_divide_internal(
            inst,
            output,
            dividend,
            divisor,
            DivisionType::Unsigned,
            KeepDivisionResult::Remainder,
        )
    }

    pub fn emit_signed_remainder(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        dividend: JIRValue,
        divisor: JIRValue,
    ) -> JITResult {
        // We can delegate to this, since divide always calculates both quotient and remainder.
        // It is just a matter of which register the output is allocated to.
        self.emit_divide_internal(inst, output, dividend, divisor, DivisionType::Signed, KeepDivisionResult::Remainder)
    }
}

fn divide_const(
    dividend: JIRConst,
    divisor: JIRConst,
) -> JIRConst {
    JIRConst::new(dividend.value().wrapping_div(divisor.value()), dividend.size())
}

fn remainder_const(
    dividend: JIRConst,
    divisor: JIRConst,
) -> JIRConst {
    JIRConst::new(dividend.value().wrapping_rem(divisor.value()), dividend.size())
}

fn divide_signed_const(
    dividend: JIRConst,
    divisor: JIRConst,
) -> JIRConst {
    let result = (dividend.value() as i128).wrapping_div(divisor.value() as i128) as u128;
    JIRConst::new(result, dividend.size())
}

fn remainder_signed_const(
    dividend: JIRConst,
    divisor: JIRConst,
) -> JIRConst {
    let result = (dividend.value() as i128).wrapping_rem(divisor.value() as i128) as u128;
    JIRConst::new(result, dividend.size())
}

#[repr(C)]
struct U128 {
    low: u64,
    high: u64,
}

unsafe extern "sysv64" fn div128(
    dividend: U128,
    divisor: U128,
    cpu_context: *mut X86TaskContext,
) -> U128 {
    // rdi = divisor.low
    // rsi = divisor.high
    // rdx = dividend.low
    // rcx = dividend.high
    // r8 = cpu_context
    // r9 not used
    let dividend: u128 = dividend.low as u128 | ((dividend.high as u128) << 64);
    let divisor: u128 = divisor.low as u128 | ((divisor.high as u128) << 64);
    let result = dividend.checked_div(divisor);
    let Some(result) = result else {
        // SAFETY: By contract, the cpu_context pointer is passed by the jit and must be valid.
        unsafe { &mut (*cpu_context) }.set_runtime_error(RuntimeError::DivisionError);
        // SAFETY: The error variable in the runtime is checked. In case of an error, the emulation will exit
        //         and the unsafe value is not used.
        // We can allow uninitialized values here since they are never used.
        #[allow(invalid_value, clippy::uninit_assumed_init)]
        return unsafe { std::mem::MaybeUninit::<U128>::uninit().assume_init() };
    };
    U128 {
        low: result as u64,
        high: (result >> 64) as u64,
    }
}

unsafe extern "sysv64" fn sdiv128(
    dividend: U128,
    divisor: U128,
    cpu_context: *mut X86TaskContext,
) -> U128 {
    // rdi = divisor.low
    // rsi = divisor.high
    // rdx = dividend.low
    // rcx = dividend.high
    // r8 = cpu_context
    // r9 not used
    let dividend = (dividend.low as u128 | ((dividend.high as u128) << 64)) as i128;
    let divisor = (divisor.low as u128 | ((divisor.high as u128) << 64)) as i128;
    let result = dividend.checked_div(divisor);
    let Some(result) = result else {
        // SAFETY: By contract, the cpu_context pointer is passed by the jit and must be valid.
        unsafe { &mut (*cpu_context) }.set_runtime_error(RuntimeError::DivisionError);
        // SAFETY: The error variable in the runtime is checked. In case of an error, the emulation will exit
        //         and the unsafe value is not used.

        // We can allow uninitialized values here since they are never used.
        #[allow(invalid_value, clippy::uninit_assumed_init)]
        return unsafe { std::mem::MaybeUninit::<U128>::uninit().assume_init() };
    };
    let result = result as u128;
    U128 {
        low: result as u64,
        high: (result >> 64) as u64,
    }
}

unsafe extern "sysv64" fn rem128(
    dividend: U128,
    divisor: U128,
    cpu_context: *mut X86TaskContext,
) -> U128 {
    // rdi = divisor.low
    // rsi = divisor.high
    // rdx = dividend.low
    // rcx = dividend.high
    // r8 = cpu_context
    // r9 not used
    let dividend: u128 = dividend.low as u128 | ((dividend.high as u128) << 64);
    let divisor: u128 = divisor.low as u128 | ((divisor.high as u128) << 64);
    let result = dividend.checked_rem(divisor);
    let Some(result) = result else {
        // SAFETY: By contract, the cpu_context pointer is passed by the jit and must be valid.
        unsafe { &mut (*cpu_context) }.set_runtime_error(RuntimeError::DivisionError);
        // SAFETY: The error variable in the runtime is checked. In case of an error, the emulation will exit
        //         and the unsafe value is not used.

        // We can allow uninitialized values here since they are never used.
        #[allow(invalid_value, clippy::uninit_assumed_init)]
        return unsafe { std::mem::MaybeUninit::<U128>::uninit().assume_init() };
    };
    U128 {
        low: result as u64,
        high: (result >> 64) as u64,
    }
}

unsafe extern "sysv64" fn srem128(
    dividend: U128,
    divisor: U128,
    cpu_context: *mut X86TaskContext,
) -> U128 {
    // rdi = dividend.low
    // rsi = dividend.high
    // rdx = divisor.low
    // rcx = divisor.high
    // r8 = cpu_context
    // r9 not used

    let dividend: u128 = dividend.low as u128 | ((dividend.high as u128) << 64);
    let divisor: u128 = divisor.low as u128 | ((divisor.high as u128) << 64);
    let result = (dividend as i128).checked_rem(divisor as i128);
    let Some(result) = result else {
        // SAFETY: By contract, the cpu_context pointer is passed by the jit and must be valid.
        unsafe { &mut (*cpu_context) }.set_runtime_error(RuntimeError::DivisionError);
        // SAFETY: The error variable in the runtime is checked. In case of an error, the emulation will exit
        //         and the unsafe value is not used.

        // We can allow uninitialized values here since they are never used.
        #[allow(invalid_value, clippy::uninit_assumed_init)]
        return unsafe { std::mem::MaybeUninit::<U128>::uninit().assume_init() };
    };
    let result = result as u128;

    U128 {
        low: result as u64,
        high: (result >> 64) as u64,
    }
}
