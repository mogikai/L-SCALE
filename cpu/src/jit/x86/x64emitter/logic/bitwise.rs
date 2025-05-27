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
use crate::jit::x86::x64emitter::copy::CopyExtendMode;
use crate::jit::x86::x64emitter::helpers::reg;
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
        match (input_a, input_b) {
            (JIRValue::Const(input_a), JIRValue::Const(input_b)) => {
                Self::copy_requirements(requirement, output, input_a);
            }
            (JIRValue::Const(input_a), JIRValue::Slice(input_b))
            | (JIRValue::Slice(input_b), JIRValue::Const(input_a)) => {
                Self::require_large_int_write_any(requirement, output);
                Self::require_large_int_read_any(requirement, input_b);
            }
            (JIRValue::Slice(input_a), JIRValue::Slice(input_b)) => {
                Self::require_large_int_write_any(requirement, output);
                Self::require_large_int_read_any(requirement, input_a);
                Self::require_large_int_read_any(requirement, input_b);
            }
        }
    }

    pub fn bitwise_negate_requirements(
        requirement: &mut AllocRequirement,
        output: JIRSlice,
        input_a: JIRValue,
    ) {
        match input_a {
            JIRValue::Slice(input) => {
                Self::require_large_int_write_any(requirement, output);
                Self::require_large_int_read_any(requirement, input);
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
        debug_assert_eq!(output.size(), input_a.size());
        debug_assert_eq!(input_a.size(), input_b.size());
        // TODO: It might make sense to directly put the memory reference into the instruction, instead of loading
        //       to a register first. However, this requires that we know if the value is used in subsequent instructions
        //       because if it is used, we have to load it in a register. The exception to this is for cases like
        //       out = out OP b, where out does not have to be loaded first and we can just use the mem reference directly.
        match (input_a, input_b) {
            (JIRValue::Const(input_a), JIRValue::Const(input_b)) => {
                // In the const case we can simply calculate the value ahead of time and move it into the output.
                self.emit_copy(inst, output, op_const(input_a, input_b))?
            }
            (JIRValue::Slice(input_slice), JIRValue::Const(input_const))
            | (JIRValue::Const(input_const), JIRValue::Slice(input_slice)) => {
                // The order of arguments does not matter, so we can simply handle both cases here
                let immediate = input_const.value();
                let immediate_size = input_const.size();

                match immediate_size {
                    ..=8 => {
                        let output_reg = reg!(self.allocations, inst, 0);
                        let input_slice_reg = reg!(self.allocations, inst, 1);
                        // Copy the input into the output, since x86 uses the first operand as output.
                        self.copy_reg(output_reg, input_slice_reg, input_slice.size(), CopyExtendMode::None)?;

                        if immediate_size <= 4 {
                            op_reg_imm(self, output_reg, immediate, immediate_size)?;
                        } else if immediate_size <= 8 {
                            let scratch = self.allocations.scratch_register(RegClass::Int);
                            self.load_const(scratch, input_const)?;
                            op_reg_reg(self, output_reg, scratch, output.size())?;
                        }
                    }
                    16 => {
                        let output_reg_lower = reg!(self.allocations, inst, 0);
                        let output_reg_upper = reg!(self.allocations, inst, 1);
                        let input_slice_reg_lower = reg!(self.allocations, inst, 2);
                        let input_slice_reg_upper = reg!(self.allocations, inst, 3);
                        self.copy_reg(output_reg_lower, input_slice_reg_lower, 8, CopyExtendMode::None)?;
                        self.copy_reg(output_reg_upper, input_slice_reg_upper, 8, CopyExtendMode::None)?;

                        let (input_const_lower, input_const_upper) = input_const.split(8);

                        let scratch = self.allocations.scratch_register(RegClass::Int);
                        self.load_const(scratch, input_const_lower)?;
                        op_reg_reg(self, output_reg_lower, scratch, 8)?;
                        self.load_const(scratch, input_const_upper)?;
                        op_reg_reg(self, output_reg_upper, scratch, 8)?;
                    }
                    size => todo!("Unsupported size {size}")
                }
            }
            (JIRValue::Slice(input_a), JIRValue::Slice(input_b)) if input_a.size() <= 8 => {
                self.emit_bitwise_op_slice_slice(inst, output, input_a, input_b, op_reg_reg)?
            }
            (JIRValue::Slice(input_a), JIRValue::Slice(input_b)) if input_a.size() == 16 => {
                self.emit_bitwise_op_slice_slice_128(inst, output, input_a, input_b, op_reg_reg)?
            }
            _ => todo!("Unsupported operands"),
        }
        Ok(())
    }

    fn emit_bitwise_op_slice_slice(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRSlice,
        input_b: JIRSlice,
        op_reg_reg: fn(&mut X64Emitter<'_>, HostReg, HostReg, JIRSize) -> JITResult,
    ) -> JITResult {
        let output_reg = reg!(self.allocations, inst, 0);
        let input_a_reg = reg!(self.allocations, inst, 1);
        let input_b_reg = reg!(self.allocations, inst, 2);
        self.emit_bitwise_op_reg_reg(output_reg, input_a_reg, input_b_reg, output.size(), op_reg_reg)
    }

    fn emit_bitwise_op_reg_reg(
        &mut self,
        output_reg: HostReg,
        input_a_reg: HostReg,
        input_b_reg: HostReg,
        size: JIRSize,
        op_reg_reg: fn(&mut X64Emitter<'_>, HostReg, HostReg, JIRSize) -> JITResult,
    ) -> JITResult {
        if output_reg == input_a_reg {
            op_reg_reg(self, output_reg, input_b_reg, size)
        } else if output_reg == input_b_reg {
            op_reg_reg(self, output_reg, input_a_reg, size)
        } else {
            self.copy_reg(output_reg, input_a_reg, size, CopyExtendMode::None)?;
            op_reg_reg(self, output_reg, input_b_reg, size)
        }
    }

    fn emit_bitwise_op_slice_slice_128(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRSlice,
        input_b: JIRSlice,
        op_reg_reg: fn(&mut X64Emitter<'_>, HostReg, HostReg, JIRSize) -> JITResult,
    ) -> JITResult {
        debug_assert_eq!(output.size(), 16);
        debug_assert_eq!(input_a.size(), 16);
        debug_assert_eq!(input_b.size(), 16);
        let output_reg_lower = reg!(self.allocations, inst, 0);
        let output_reg_upper = reg!(self.allocations, inst, 1);
        let input_a_reg_lower = reg!(self.allocations, inst, 2);
        let input_a_reg_upper = reg!(self.allocations, inst, 3);
        let input_b_reg_lower = reg!(self.allocations, inst, 4);
        let input_b_reg_upper = reg!(self.allocations, inst, 5);

        self.emit_bitwise_op_reg_reg(output_reg_lower, input_a_reg_lower, input_b_reg_lower, 8, op_reg_reg)?;
        self.emit_bitwise_op_reg_reg(output_reg_upper, input_a_reg_upper, input_b_reg_upper, 8, op_reg_reg)
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
            JIRValue::Const(value) => {
                // In the const case we can simply calculate the value ahead of time and move it into the output.
                self.emit_copy(inst, output, JIRConst::new(!value.value(), value.size()))?;
            }
            JIRValue::Slice(input) => match output.size() {
                ..=8 => {
                    let output_reg = reg!(self.allocations, inst, 0);
                    let input_reg = reg!(self.allocations, inst, 1);

                    // Copy into output, since neg uses the same register for input and output
                    self.copy_reg(output_reg, input_reg, output.size(), CopyExtendMode::None);
                    assemble!(self.block.assembler();
                    ; not Rq(output_reg.hw_enc())
                    );
                }
                16 => {
                    let output_reg_lower = reg!(self.allocations, inst, 0);
                    let output_reg_upper = reg!(self.allocations, inst, 1);
                    let input_reg_lower = reg!(self.allocations, inst, 2);
                    let input_reg_upper = reg!(self.allocations, inst, 3);

                    // Copy into output, since neg uses the same register for input and output
                    self.copy_reg(output_reg_lower, input_reg_lower, 8, CopyExtendMode::None);
                    self.copy_reg(output_reg_upper, input_reg_upper, 8, CopyExtendMode::None);
                    assemble!(self.block.assembler();
                    ; not Rq(output_reg_lower.hw_enc())
                    ; not Rq(output_reg_upper.hw_enc())
                    );
                }
                size => todo!("Unsupported size {size}"),
            },
        }

        Ok(())
    }
}
