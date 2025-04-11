use crate::jit::api::JITResult;
use crate::jit::host::register::{
    HostRegSet,
    RegClass,
};
use crate::jit::regalloc::requirements::OperandRequest;
use crate::jit::x86::alloc_requirement::AllocRequirement;
use crate::jit::x86::assemble;
use crate::jit::x86::registers::general_purpose::GPRegister;
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
                if dividend.size() > 8 {
                    todo!("Unsupported divide size")
                }
                requirement.add_request(OperandRequest::int_read_any(dividend));
            }
            (JIRValue::Slice(dividend), JIRValue::Slice(divisor)) => {
                Self::load_output(requirement, output, keep);
                if dividend.size() > 8 {
                    todo!("Unsupported divide size")
                }
                requirement.add_request(OperandRequest::int_read_any(dividend));
                requirement.add_request(OperandRequest::int_read_any(divisor));
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
            "Expected the output to be in AX or DX, not in {}",
            output_reg
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
