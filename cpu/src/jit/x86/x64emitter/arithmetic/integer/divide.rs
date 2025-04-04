use crate::jit::api::JITResult;
use crate::jit::code_buffer::CompiledBasicBlock;
use crate::jit::host::register::{
    HostRegSet,
    RegClass,
};
use crate::jit::regalloc::requirements::{
    OperandRequest,
    TempRequest,
};
use crate::jit::regalloc::result::Allocation;
use crate::jit::x86::alloc_requirement::AllocRequirement;
use crate::jit::x86::registers::general_purpose::GPRegister;
use crate::jit::x86::x64emitter::regalloc::SliceType;
use crate::jit::x86::x64emitter::{
    X64AllocRequirements,
    X64Emitter,
};
use crate::jit::x86::{
    X86MogiJit,
    assemble,
};
use dynasmrt::x64::X64Relocation;
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

impl X64AllocRequirements {
    pub fn divide_requirements(
        requirement: &mut AllocRequirement,
        output: JIRSlice,
        dividend: JIRValue,
        divisor: JIRValue,
    ) {
        match (dividend, divisor) {
            (JIRValue::Const(dividend), JIRValue::Const(divisor)) => {
                Self::copy_requirements(requirement, output, divide_const(dividend, divisor));
            }
            (JIRValue::Const(dividend), JIRValue::Slice(divisor)) => {
                // Dividend is loaded into RDX:RAX
                *requirement.clobbers_mut() = HostRegSet::empty()
                    .with(GPRegister::AX.into_host_reg())
                    .with(GPRegister::DX.into_host_reg());
                requirement.add_request(OperandRequest::int_write_any(output));
                requirement.add_request(OperandRequest::int_read_any(divisor));
            }
            (JIRValue::Slice(dividend), JIRValue::Const(divisor)) => {
                *requirement.clobbers_mut() = HostRegSet::empty().with(GPRegister::DX.into_host_reg());
                requirement.add_request(OperandRequest::int_write_any(output));
                // The dividend must always be in RDX:RAX.
                // Currently we only load RAX, since we only support 64 bits.
                // TODO: Support more than 64 bit dividend
                requirement.add_request(OperandRequest::int_read_fixed_reg(GPRegister::AX.into_host_reg(), dividend));
                *requirement.required_temps_mut() = TempRequest::new().int(1);
            }
            (JIRValue::Slice(dividend), JIRValue::Slice(divisor)) => {
                *requirement.clobbers_mut() = HostRegSet::empty().with(GPRegister::DX.into_host_reg());
                requirement.add_request(OperandRequest::int_write_any(output));
                // The dividend must always be in RDX:RAX.
                // Currently we only load RAX, since we only support 64 bits.
                // TODO: Support more than 64 bit dividend
                requirement.add_request(OperandRequest::int_read_fixed_reg(GPRegister::AX.into_host_reg(), dividend));
                requirement.add_request(OperandRequest::int_read_any(divisor));
            }
        }
    }

    pub fn divide_signed_requirements(
        requirement: &mut AllocRequirement,
        output: JIRSlice,
        dividend: JIRValue,
        divisor: JIRValue,
    ) {
        match (dividend, divisor) {
            (JIRValue::Const(dividend), JIRValue::Const(divisor)) => {
                Self::copy_requirements(requirement, output, divide_const(dividend, divisor));
            }
            (dividend, divisor) => Self::divide_requirements(requirement, output, dividend, divisor),
        }
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
        assert_eq!(dividend.size(), divisor.size());
        assert_eq!(dividend.size(), output.size());
        if let (JIRValue::Const(dividend), JIRValue::Const(divisor)) = (dividend, divisor) {
            // If both arguments are const we can emit a move of the calculated value.
            return self.emit_copy_deprecated(output, divide_const(dividend, divisor));
        }
        self.emit_divide_value_value(inst, DivisionType::Unsigned, output, dividend.into(), divisor)?;
        Ok(())
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
        let Allocation::Register(output_reg) = self.allocations.get_allocation(inst, 0) else {
            todo!()
        };
        // Div expects the dividend in D:A (or just AX for word/byte size)
        // Since we only deal with 64-bit numbers currently, it is enough to load A and zero D
        let mut divisor_operand_id = 1;
        let dividend_upper_reg = GPRegister::DX.into_host_reg();
        let dividend_lower_reg = match dividend {
            JIRValue::Slice(_) => {
                // The dividend was also allocated, so the divisor is located in the third slot.
                divisor_operand_id += 1;
                let Allocation::Register(dividend_reg) = self.allocations.get_allocation(inst, 1) else {
                    panic!("Regalloc bug. We requested a register for the dividend, but did not get one.")
                };
                assert_eq!(dividend_reg, GPRegister::AX.into_host_reg(), "Expected the dividend to be in AX");
                dividend_reg
            }
            JIRValue::Const(dividend) => {
                self.load_const(GPRegister::AX.into_host_reg(), dividend)?;
                GPRegister::AX.into_host_reg()
            }
        };
        match division_type {
            DivisionType::Signed => {
                // Sign extend rax to rdx:rax
                // TODO: Use cwd or cdq for other sizes?
                assemble!(self.block.assembler();
                    ; cqo
                );
            }
            DivisionType::Unsigned => {
                // Set the upper part of the dividend to zero, since we are only dealing with 64 bit values here.
                self.load_zero(dividend_upper_reg, dividend.size())?;
            }
        }

        // Fetch the divisor register and load the value.
        let divisor_reg = match divisor {
            JIRValue::Slice(_) => {
                let Allocation::Register(divisor_reg) = self.allocations.get_allocation(inst, divisor_operand_id)
                else {
                    todo!()
                };
                divisor_reg
            }
            JIRValue::Const(divisor_const) => {
                let tmp = self
                    .allocations
                    .get_temp_allocations(inst)
                    .get_nth_from_class(0, RegClass::Int)
                    .expect("We requested this");
                self.load_const(tmp, divisor_const)?;
                tmp
            }
        };

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
        // Finally we can copy over the result into the output if they are in different registers.
        if output_reg != dividend_lower_reg {
            assemble!(self.block.assembler();
                // dividend_lower contains the quotient
                // dividend_upper contains the remainder, which we do not need here.
                ; mov Rq(output_reg.hw_enc()), Rq(dividend_lower_reg.hw_enc())
            );
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
        match (dividend, divisor) {
            (JIRValue::Const(dividend), JIRValue::Const(divisor)) => {
                return self.emit_copy(inst, output, divide_signed_const(dividend, divisor));
            }
            _ => {}
        }
        self.emit_divide_value_value(inst, DivisionType::Signed, output, dividend, divisor)
    }
}

fn divide_const(
    dividend: JIRConst,
    divisor: JIRConst,
) -> JIRConst {
    JIRConst::new(dividend.value().wrapping_div(divisor.value()), dividend.size())
}

fn divide_signed_const(
    dividend: JIRConst,
    divisor: JIRConst,
) -> JIRConst {
    let result = (dividend.value() as i128).wrapping_div(divisor.value() as i128) as u128;
    JIRConst::new(result, dividend.size())
}
