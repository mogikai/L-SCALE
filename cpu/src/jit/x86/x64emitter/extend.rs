use crate::jit::api::JITResult;
use crate::jit::regalloc::requirements::OperandRequest;
use crate::jit::regalloc::result::Allocation;
use crate::jit::x86::alloc_requirement::AllocRequirement;
use crate::jit::x86::x64emitter::regalloc::SliceType;
use crate::jit::x86::x64emitter::slice_load::CopyExtendMode;
use crate::jit::x86::x64emitter::{
    X64AllocRequirements,
    X64Emitter,
};
use mogi_sleigh::spec::jir::basic_block::InstId;
use mogi_sleigh::spec::jir::values::{
    JIRSlice,
    JIRValue,
};

impl X64AllocRequirements {
    pub fn zero_extend_requirements(
        requirement: &mut AllocRequirement,
        output: JIRSlice,
        input: JIRValue,
    ) {
        requirement.add_request(OperandRequest::int_write_any(output));
        input.is_slice_then(|s| requirement.add_request(OperandRequest::int_read_any(*s)));
    }

    pub fn sign_extend_requirements(
        requirement: &mut AllocRequirement,
        output: JIRSlice,
        input: JIRValue,
    ) {
        requirement.add_request(OperandRequest::int_write_any(output));
        input.is_slice_then(|s| requirement.add_request(OperandRequest::int_read_any(*s)));
    }
}

impl X64Emitter<'_> {
    pub fn emit_zero_extend(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input: JIRValue,
    ) -> JITResult {
        let Allocation::Register(output_reg) = self.allocations.get_allocation(inst, 0) else {
            todo!()
        };
        match input {
            JIRValue::Slice(slice) => {
                let Allocation::Register(input_reg) = self.allocations.get_allocation(inst, 1) else {
                    todo!()
                };
                self.copy_reg(output_reg, input_reg, input.size(), CopyExtendMode::ZeroExtend(output.size()))?;
            }
            JIRValue::Const(value) => {
                let value = value.zext(output.size()).expect("Invalid zero extend");
                self.load_const(output_reg, value)?;
            }
        }
        Ok(())
    }

    pub fn emit_sign_extend(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input: JIRValue,
    ) -> JITResult {
        let Allocation::Register(output_reg) = self.allocations.get_allocation(inst, 0) else {
            todo!()
        };
        match input {
            JIRValue::Slice(slice) => {
                let Allocation::Register(input_reg) = self.allocations.get_allocation(inst, 1) else {
                    todo!()
                };
                self.copy_reg(output_reg, input_reg, input.size(), CopyExtendMode::SignExtend(output.size()))?;
            }
            JIRValue::Const(value) => {
                // For const values we simply copy
                // let value = value.zext(output.size()).expect("Invalid zero extend");
                todo!()
                // self.emit_copy(output, value)?;
            }
        }
        Ok(())
    }
}
