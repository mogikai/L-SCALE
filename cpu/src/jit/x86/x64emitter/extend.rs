use crate::jit::api::JITResult;
use crate::jit::regalloc::requirements::OperandRequest;
use crate::jit::regalloc::result::Allocation;
use crate::jit::x86::alloc_requirement::AllocRequirement;
use crate::jit::x86::assemble;
use crate::jit::x86::registers::general_purpose::GPRegister;
use crate::jit::x86::x64emitter::regalloc::SliceType;
use crate::jit::x86::x64emitter::slice_load::CopyExtendMode;
use crate::jit::x86::x64emitter::{
    X64AllocRequirements,
    X64Emitter,
};
use mogi_sleigh::spec::jir::basic_block::InstId;
use mogi_sleigh::spec::jir::common::JIRSize;
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
        match output.size() {
            size if size <= 8 => {
                requirement.add_request(OperandRequest::int_write_any(output));
                input.is_slice_then(|s| requirement.add_request(OperandRequest::int_read_any(*s)));
            }
            size if size == 16 => {
                // If we are extending to 16 bytes, the output must be put in RDX:RAX
                requirement
                    .add_request(OperandRequest::int_write_fixed(GPRegister::AX.into_host_reg(), output.slice(0, 8)));
                requirement
                    .add_request(OperandRequest::int_write_fixed(GPRegister::DX.into_host_reg(), output.slice(8, 8)));
                input.is_slice_then(|s| requirement.add_request(OperandRequest::int_read_reg(*s)));
            }
            _ => todo!("unsupported size"),
        }
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
                match output.size() {
                    size if size <= 8 => {
                        let Allocation::Register(input_reg) = self.allocations.get_allocation(inst, 1) else {
                            todo!()
                        };
                        self.copy_reg(output_reg, input_reg, input.size(), CopyExtendMode::SignExtend(output.size()))?;
                    }
                    size if size == 16 => {
                        // Input is in slot 2, because we have two output registers.
                        let Allocation::Register(input_reg) = self.allocations.get_allocation(inst, 2) else {
                            todo!()
                        };
                        // If the size is 16, the outputs are in RDX:RAX.
                        // The input is also RAX, so we need to copy the input there.
                        self.copy_reg(GPRegister::AX.into_host_reg(), input_reg, input.size(), CopyExtendMode::None)?;
                        assemble!(self.block.assembler();
                            ; cqo
                        );
                    }
                    _ => todo!("unsupported size"),
                }
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
