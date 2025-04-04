use crate::jit::api::{
    JITResult,
    JitError,
};
use crate::jit::host::register::{
    HostReg,
    HostRegSet,
    RegClass,
};
use crate::jit::regalloc::requirements::{
    OperandRequest,
    TempRequest,
};
use crate::jit::regalloc::result::Allocation;
use crate::jit::x86::alloc_requirement::AllocRequirement;
use crate::jit::x86::assemble;
use crate::jit::x86::x64emitter::regalloc::SliceType;
use crate::jit::x86::x64emitter::slice_load::CopyExtendMode;
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
use std::cmp::min;

impl X64AllocRequirements {
    pub fn multiply_requirements(
        requirement: &mut AllocRequirement,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) {
        match (input_a, input_b) {
            (JIRValue::Const(input_a), JIRValue::Const(input_b)) => {
                Self::copy_requirements(requirement, output, const_multiply(input_a, input_b))
            }
            (JIRValue::Slice(input_a), JIRValue::Const(input_b))
            | (JIRValue::Const(input_b), JIRValue::Slice(input_a))
                if input_b.value() == 0 =>
            {
                Self::copy_requirements(requirement, output, input_b)
            }
            (JIRValue::Slice(input_a), JIRValue::Const(input_b))
            | (JIRValue::Const(input_b), JIRValue::Slice(input_a)) => {
                requirement.add_request(OperandRequest::int_write_any(output));
                requirement.add_request(OperandRequest::int_read_any(input_a));
                if immediate_size(&input_b) >= 8 {
                    *requirement.required_temps_mut() = TempRequest::new().int(1);
                }
            }
            (JIRValue::Slice(input_a), JIRValue::Slice(input_b)) => {
                requirement.add_request(OperandRequest::int_write_any(output));
                requirement.add_request(OperandRequest::int_read_any(input_a));
                requirement.add_request(OperandRequest::int_read_any(input_b));
            }
        }
    }
}

impl X64Emitter<'_> {
    #[inline]
    pub fn emit_multiply(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        match (input_a, input_b) {
            (JIRValue::Const(input_a), JIRValue::Const(input_b)) => {
                self.emit_multiply_const_const(inst, output, input_a, input_b)?;
            }
            (JIRValue::Slice(input_a), JIRValue::Const(input_b))
            | (JIRValue::Const(input_b), JIRValue::Slice(input_a)) => {
                // Since multiplication is commutative, we can just swap the slice and const arguments
                self.emit_multiply_slice_const(inst, output, input_a, input_b)?;
            }
            (JIRValue::Slice(input_a), JIRValue::Slice(input_b)) => {
                self.emit_multiply_slice_slice(inst, output, input_a, input_b)?;
            }
        }
        Ok(())
    }

    #[inline]
    fn emit_multiply_const_const(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRConst,
        input_b: JIRConst,
    ) -> Result<(), JitError> {
        debug_assert_eq!(output.size(), input_a.size());
        debug_assert_eq!(input_a.size(), input_b.size());
        self.emit_copy(inst, output, const_multiply(input_a, input_b))?;
        Ok(())
    }

    #[inline]
    fn emit_multiply_slice_const(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRSlice,
        input_b: JIRConst,
    ) -> Result<(), JitError> {
        if input_b.value() == 0 {
            // We can just zero the target, since multiplying by zero always results in zero.
            return self.emit_copy(inst, output, input_b);
        }

        let Allocation::Register(output_reg) = self.allocations.get_allocation(inst, 0) else {
            todo!()
        };
        let Allocation::Register(input_a_reg) = self.allocations.get_allocation(inst, 1) else {
            todo!()
        };
        // The size in bytes needed for the immediate
        let imm_size = immediate_size(&input_b);
        match imm_size {
            1 => {
                // We can encode the constant as immediate if it fits into at most 4  bytes.
                let immediate = input_b.value();
                assemble!(self.block.assembler();
                    ; imul Rq(output_reg.hw_enc()), Rq(input_a_reg.hw_enc()), BYTE immediate as _
                );
            }
            2 => {
                // We can encode the constant as immediate if it fits into at most 4  bytes.
                let immediate = input_b.value();
                assemble!(self.block.assembler();
                    ; imul Rq(output_reg.hw_enc()), Rq(input_a_reg.hw_enc()), WORD immediate as _
                );
            }
            3..=4 => {
                // We can encode the constant as immediate if it fits into at most 4  bytes.
                let immediate = input_b.value();
                assemble!(self.block.assembler();
                    ; imul Rq(output_reg.hw_enc()), Rq(input_a_reg.hw_enc()), DWORD immediate as _
                );
            }
            5..=8 => {
                // We need to copy the input to the output, since imul reg, reg uses the first argument as
                // input and output argument.
                let temp = self
                    .allocations
                    .get_temp_allocations(inst)
                    .get_nth_from_class(0, RegClass::Int)
                    .expect("We requested this");
                self.load_const(temp, input_b)?;
                if output_reg != input_a_reg {
                    // TODO: Can we task the reg allocator to put the input into the output beforehand?
                    self.copy_reg(output_reg, input_a_reg, output.size(), CopyExtendMode::None)?;
                }
                assemble!(self.block.assembler();
                    ; imul Rq(output_reg.hw_enc()), Rq(temp.hw_enc())
                );
            }
            _ => {
                todo!("Currently unsupported size for multiply: {}", input_b.size())
            }
        }
        Ok(())
    }

    #[inline]
    fn emit_multiply_slice_slice(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRSlice,
        input_b: JIRSlice,
    ) -> Result<(), JitError> {
        let Allocation::Register(output_reg) = self.allocations.get_allocation(inst, 0) else {
            todo!()
        };
        let Allocation::Register(input_a_reg) = self.allocations.get_allocation(inst, 1) else {
            todo!()
        };
        let Allocation::Register(input_b_reg) = self.allocations.get_allocation(inst, 2) else {
            todo!()
        };

        if output_reg == input_a_reg {
            // input_a is already in output, so we just multiply by input_b
            self.reg_mul(output_reg, input_b_reg);
        } else if output_reg == input_b_reg {
            // input_b is already in output, so we just multiply it by input_a
            self.reg_mul(output_reg, input_a_reg);
        } else {
            // Copy input_a into the output and then multiply it by input_b
            self.copy_reg(output_reg, input_a_reg, output.size(), CopyExtendMode::None)?;
            self.reg_mul(output_reg, input_b_reg);
        }
        Ok(())
    }

    fn reg_mul(
        &mut self,
        in_out: HostReg,
        input: HostReg,
    ) {
        assemble!(self.block.assembler();
            ; imul Rq(in_out.hw_enc()), Rq(input.hw_enc())
        );
    }
}

fn const_multiply(
    a: JIRConst,
    b: JIRConst,
) -> JIRConst {
    JIRConst::new(a.value().wrapping_mul(b.value()) % (1u128 << (a.size() * 8)), a.size())
}

fn immediate_size(value: &JIRConst) -> u32 {
    min((u128::BITS - value.value().leading_zeros() + 7) / 8, value.size().into())
}
