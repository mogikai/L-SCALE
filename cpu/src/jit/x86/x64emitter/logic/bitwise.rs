use crate::jit::api::JITResult;
use crate::jit::host::register::{
    HostReg,
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
use mogi_sleigh::spec::jir::common::JIRSize;
use mogi_sleigh::spec::jir::values::{
    JIRConst,
    JIRSlice,
    JIRValue,
};

impl X64AllocRequirements {
    pub fn bitwise_requirements(
        requirement: &mut AllocRequirement,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) {
        requirement.add_request(OperandRequest::int_write_any(output));

        match (input_a, input_b) {
            (JIRValue::Const(input_a), JIRValue::Const(input_b)) => {
                Self::copy_requirements(requirement, output, input_a);
            }
            (JIRValue::Const(input_a), JIRValue::Slice(input_b))
            | (JIRValue::Slice(input_b), JIRValue::Const(input_a)) => {
                requirement.add_request(OperandRequest::int_read_any(input_b));
                if input_a.size() >= 8 {
                    *requirement.required_temps_mut() = TempRequest::new().int(1);
                }
            }
            (JIRValue::Slice(input_a), JIRValue::Slice(input_b)) => {
                requirement.add_request(OperandRequest::int_read_any(input_a));
                requirement.add_request(OperandRequest::int_read_any(input_b));
            }
        }
    }

    pub fn bitwise_negate_requirements(
        requirement: &mut AllocRequirement,
        output: JIRSlice,
        input_a: JIRValue,
    ) {
        match input_a {
            JIRValue::Slice(jirslice) => {
                requirement.add_request(OperandRequest::int_write_any(output));
                requirement.add_request(OperandRequest::int_read_any(jirslice));
            }
            JIRValue::Const(jirconst) => {
                Self::copy_requirements(requirement, output, input_a);
            }
        }
    }
}

impl X64Emitter<'_> {
    pub fn emit_bitwise_and(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        self.emit_bitwise_op(
            inst,
            output,
            input_a,
            input_b,
            |a, b| JIRConst::new(a.value() & b.value(), a.size()),
            |emitter, output_gp, immediate, size| {
                // Choose the smallest encoding that still fits
                match size {
                    0 => {
                        panic!("Zero sized slices are not allowed")
                    }
                    1 => {
                        assemble!(emitter.block.assembler();
                            ; and Rq(output_gp.hw_enc()), BYTE immediate as _
                        );
                    }
                    2 => {
                        assemble!(emitter.block.assembler();
                            ; and Rq(output_gp.hw_enc()), WORD immediate as _
                        );
                    }
                    3..=4 => {
                        assemble!(emitter.block.assembler();
                            ; and Rq(output_gp.hw_enc()), DWORD immediate as _
                        );
                    }
                    _ => {
                        unreachable!("This code is not reachable because we checked beforehand, that the size is <= 4")
                    }
                }
                Ok(())
            },
            |emitter, output_gp, operand_gp, _size| {
                assemble!(emitter.block.assembler();
                    ; and Rq(output_gp.hw_enc()), Rq(operand_gp.hw_enc())
                );
                Ok(())
            },
        )
    }

    pub fn emit_bitwise_or(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        self.emit_bitwise_op(
            inst,
            output,
            input_a,
            input_b,
            |a, b| JIRConst::new(a.value() | b.value(), a.size()),
            |emitter, output_gp, immediate, size| {
                // Choose the smallest encoding that still fits
                match size {
                    0 => {
                        panic!("Zero sized slices are not allowed")
                    }
                    1 => {
                        assemble!(emitter.block.assembler();
                            ; or Rq(output_gp.hw_enc()), BYTE immediate as _
                        );
                    }
                    2 => {
                        assemble!(emitter.block.assembler();
                            ; or Rq(output_gp.hw_enc()), WORD immediate as _
                        );
                    }
                    3..=4 => {
                        assemble!(emitter.block.assembler();
                            ; or Rq(output_gp.hw_enc()), DWORD immediate as _
                        );
                    }
                    _ => {
                        unreachable!("This code is not reachable because we checked beforehand, that the size is <= 4")
                    }
                }
                Ok(())
            },
            |emitter, output_gp, operand_gp, _size| {
                assemble!(emitter.block.assembler();
                    ; or Rq(output_gp.hw_enc()), Rq(operand_gp.hw_enc())
                );
                Ok(())
            },
        )
    }

    pub fn emit_bitwise_xor(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        self.emit_bitwise_op(
            inst,
            output,
            input_a,
            input_b,
            |a, b| JIRConst::new(a.value() ^ b.value(), a.size()),
            |emitter, output_reg, immediate, size| {
                // Choose the smallest encoding that still fits
                match size {
                    0 => {
                        panic!("Zero sized slices are not allowed")
                    }
                    1 => {
                        assemble!(emitter.block.assembler();
                            ; xor Rq(output_reg.hw_enc()), BYTE immediate as _
                        );
                    }
                    2 => {
                        assemble!(emitter.block.assembler();
                            ; xor Rq(output_reg.hw_enc()), WORD immediate as _
                        );
                    }
                    3..=4 => {
                        assemble!(emitter.block.assembler();
                            ; xor Rq(output_reg.hw_enc()), DWORD immediate as _
                        );
                    }
                    _ => {
                        unreachable!("This code is not reachable because we checked beforehand, that the size is <= 4")
                    }
                }
                Ok(())
            },
            |emitter, output_reg, operand_reg, _size| {
                assemble!(emitter.block.assembler();
                    ; xor Rq(output_reg.hw_enc()), Rq(operand_reg.hw_enc())
                );
                Ok(())
            },
        )
    }

    pub fn emit_bitwise_op(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
        op_const: fn(JIRConst, JIRConst) -> JIRConst,
        op_reg_imm: fn(&mut X64Emitter<'_>, HostReg, u128, JIRSize) -> JITResult,
        op_reg_reg: fn(&mut X64Emitter<'_>, HostReg, HostReg, JIRSize) -> JITResult,
    ) -> JITResult {
        assert_eq!(input_a.size(), input_b.size());
        // TODO: It might make sense to directly put the memory reference into the instruction, instead of loading
        //       to a register first. However, this requires that we know if the value is used in subsequent instructions
        //       because if it is used, we have to load it in a register. The exception to this is for cases like
        //       out = out OP b, where out does not have to be loaded first and we can just use the mem reference directly.
        match (input_a, input_b) {
            (JIRValue::Const(input_a), JIRValue::Const(input_b)) => {
                // In the const case we can simply calculate the value ahead of time and move it into the output.
                self.emit_copy(inst, output, op_const(input_a, input_b))?
            }
            (JIRValue::Slice(input_a), JIRValue::Const(input_b))
            | (JIRValue::Const(input_b), JIRValue::Slice(input_a)) => {
                // The order of arguments does not matter, so we can simply handle both cases here
                let immediate = input_b.value();
                let immediate_size = input_b.size();

                let Allocation::Register(output_reg) = self.allocations.get_allocation(inst, 0) else {
                    todo!()
                };

                let Allocation::Register(input_a_reg) = self.allocations.get_allocation(inst, 1) else {
                    todo!()
                };

                if immediate_size <= 4 {
                    // Copy the input into the output, since x86 uses the first operand as output.

                    self.copy_reg(output_reg, input_a_reg, input_a.size(), CopyExtendMode::None)?;
                    op_reg_imm(self, output_reg, immediate, immediate_size)?;
                } else if immediate_size <= 8 {
                    let tempreg = self
                        .allocations
                        .get_temp_allocations(inst)
                        .get_nth_from_class(0, RegClass::Int)
                        .expect("We allocated this above");

                    let Allocation::Register(input_a_reg) = self.allocations.get_allocation(inst, 1) else {
                        todo!()
                    };

                    self.copy_reg(output_reg, input_a_reg, output.size(), CopyExtendMode::None)?;
                    self.load_const(tempreg, input_b)?;
                    op_reg_reg(self, output_reg, tempreg, output.size())?;
                } else {
                    todo!("Unsupported operation size")
                }
            }
            (JIRValue::Slice(input_a), JIRValue::Slice(input_b)) => {
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
                    op_reg_reg(self, output_reg, input_b_reg, output.size())?;
                } else if output_reg == input_b_reg {
                    op_reg_reg(self, output_reg, input_a_reg, output.size())?;
                } else {
                    self.copy_reg(output_reg, input_a_reg, output.size(), CopyExtendMode::None)?;
                    op_reg_reg(self, output_reg, input_b_reg, output.size())?;
                }
            }
        }
        Ok(())
    }

    pub fn emit_bitwise_negate(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRValue,
    ) -> JITResult {
        assert_eq!(output.size(), input_a.size());
        // TODO: It might make sense to directly put the memory reference into the instruction, instead of loading
        //       to a register first. However, this requires that we know if the value is used in subsequent instructions
        //       because if it is used, we have to load it in a register. The exception to this is for cases like
        //       out = out OP b, where out does not have to be loaded first and we can just use the mem reference directly.

        match input_a {
            JIRValue::Const(input_a) => {
                // In the const case we can simply calculate the value ahead of time and move it into the output.
                self.emit_copy(inst, output, JIRConst::new(!input_a.value(), input_a.size()))?;
            }
            JIRValue::Slice(input_a) => {
                let Allocation::Register(output_reg) = self.allocations.get_allocation(inst, 0) else {
                    todo!()
                };

                let Allocation::Register(input_a_reg) = self.allocations.get_allocation(inst, 1) else {
                    todo!()
                };

                self.copy_reg(output_reg, input_a_reg, output.size(), CopyExtendMode::None)?;
                assemble!(self.block.assembler();
                    ; not Rq(output_reg.hw_enc())
                );
            }
        }
        Ok(())
    }
}
