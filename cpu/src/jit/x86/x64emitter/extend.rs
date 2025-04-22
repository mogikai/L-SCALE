use crate::jit::api::JITResult;
use crate::jit::regalloc::requirements::OperandRequest;
use crate::jit::regalloc::result::Allocation;
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
        match output.size() {
            size if size <= 8 => requirement.add_request(OperandRequest::int_write_any(output)),
            size if size == 16 => {
                let (output_lower, output_upper) = output.split(8);
                requirement.add_request(OperandRequest::int_write_any(output_lower));
                requirement.add_request(OperandRequest::int_write_any(output_upper));
            }
            _ => todo!("unsupported size"),
        }
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
            size if size == 16 && input.size() <= 8 => {
                // If we are extending to 16 bytes, the output must be put in RDX:RAX
                Self::require_large_int_write_fixed(
                    requirement,
                    output,
                    GPRegister::AX.into_host_reg(),
                    GPRegister::DX.into_host_reg(),
                );
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
        let output_reg = reg!(self.allocations, inst, 0);
        match input {
            JIRValue::Slice(slice) if output.size() <= 8 => {
                let input_reg = reg!(self.allocations, inst, 1);
                self.copy_reg(output_reg, input_reg, input.size(), CopyExtendMode::ZeroExtend(output.size()))?;
            }
            JIRValue::Slice(slice) if output.size() == 16 => {
                let output_upper_reg = reg!(self.allocations, inst, 1);
                let input_reg = reg!(self.allocations, inst, 2);
                self.copy_reg(output_reg, input_reg, input.size(), CopyExtendMode::ZeroExtend(8))?;
                self.load_zero(output_upper_reg, 8)?;
            }
            JIRValue::Const(value) => {
                let value = value.zext(output.size()).expect("Invalid zero extend");
                self.load_const(output_reg, value)?;
            }
            _ => todo!("Unsupported operands"),
        }
        Ok(())
    }

    pub fn emit_sign_extend(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input: JIRValue,
    ) -> JITResult {
        match input {
            JIRValue::Slice(slice) => {
                match output.size() {
                    size if size <= 8 => {
                        let output_reg = reg!(self.allocations, inst, 0);
                        let input_reg = reg!(self.allocations, inst, 1);
                        self.copy_reg(output_reg, input_reg, input.size(), CopyExtendMode::SignExtend(output.size()))?;
                    }
                    size if size == 16 => {
                        // Input is in slot 2, because we have two fixed output registers.
                        let input_reg = reg!(self.allocations, inst, 2);
                        // If the size is 16, the outputs are in RDX:RAX.
                        // The input is also RAX, so we need to copy the input there.
                        self.copy_reg(
                            GPRegister::AX.into_host_reg(),
                            input_reg,
                            input.size(),
                            CopyExtendMode::SignExtend(8),
                        )?;
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
