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

enum FlagType {
    Overflow,
    Carry,
    Zero,
}

impl X64AllocRequirements {
    pub fn carry_requirements(
        requirement: &mut AllocRequirement,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) {
        match (input_a, input_b) {
            (JIRValue::Const(input_a), JIRValue::Const(input_b)) => {
                // Add copy requirements for a dummy value with size one since we delegate to copy.
                Self::copy_requirements(requirement, output, JIRConst::new(0, 1));
            }
            (JIRValue::Slice(input_a), JIRValue::Const(input_b))
            | (JIRValue::Const(input_b), JIRValue::Slice(input_a)) => {
                requirement.add_request(OperandRequest::int_write_any(output));
                requirement.add_request(OperandRequest::int_write_any(input_a));
                if input_b.size() >= 8 {
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

    pub fn scarry_requirements(
        requirement: &mut AllocRequirement,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) {
        match (input_a, input_b) {
            (JIRValue::Const(input_a), JIRValue::Const(input_b)) => {
                // Add copy requirements for a dummy value with size one since we delegate to copy.
                Self::copy_requirements(requirement, output, JIRConst::new(0, 1));
            }
            (JIRValue::Slice(input_a), JIRValue::Const(input_b))
            | (JIRValue::Const(input_b), JIRValue::Slice(input_a)) => {
                requirement.add_request(OperandRequest::int_write_any(output));
                requirement.add_request(OperandRequest::int_write_any(input_a));
                if input_b.size() >= 8 {
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

    pub fn sborrow_requirements(
        requirement: &mut AllocRequirement,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) {
        match (input_a, input_b) {
            (JIRValue::Const(input_a), JIRValue::Const(input_b)) => {
                // Add copy requirements for a dummy value with size one since we delegate to copy.
                Self::copy_requirements(requirement, output, JIRConst::new(0, 1));
            }
            (JIRValue::Slice(input_a), JIRValue::Const(input_b))
            | (JIRValue::Const(input_b), JIRValue::Slice(input_a)) => {
                requirement.add_request(OperandRequest::int_write_any(output));
                requirement.add_request(OperandRequest::int_write_any(input_a));
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
    pub fn emit_scarry(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        assert_eq!(output.size(), 1);
        assert_eq!(input_a.size(), input_b.size());
        match (input_a, input_b) {
            (JIRValue::Const(input_a), JIRValue::Const(input_b)) => self.emit_add_flag_const_const_signed(
                inst,
                output,
                input_a.value() as i128,
                input_b.value() as i128,
                input_a.size(),
            ),
            (JIRValue::Slice(input_a), JIRValue::Const(input_b))
            | (JIRValue::Const(input_b), JIRValue::Slice(input_a)) => {
                self.emit_add_flag_slice_const(inst, output, input_a, input_b, FlagType::Overflow)
            }
            (JIRValue::Slice(input_a), JIRValue::Slice(input_b)) => {
                self.emit_add_flag_slice_slice(inst, output, input_a, input_b, FlagType::Overflow)
            }
        }
    }

    pub fn emit_sborrow(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        assert_eq!(output.size(), 1);
        assert_eq!(input_a.size(), input_b.size());
        match (input_a, input_b) {
            (JIRValue::Const(input_a), JIRValue::Const(input_b)) => self.emit_sub_flag_const_const_signed(
                inst,
                output,
                input_a.value() as i128,
                input_b.value() as i128,
                input_a.size(),
            ),
            (JIRValue::Slice(input_a), JIRValue::Const(input_b)) => {
                self.emit_sub_flag_slice_const(inst, output, input_a, input_b, FlagType::Overflow)
            }
            (input_a, JIRValue::Slice(input_b)) => {
                self.emit_sub_flag_value_slice(inst, output, input_a, input_b, FlagType::Overflow)
            }
        }
    }

    pub fn emit_carry(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        assert_eq!(output.size(), 1);
        assert_eq!(input_a.size(), input_b.size());
        match (input_a, input_b) {
            (JIRValue::Const(input_a), JIRValue::Const(input_b)) => {
                self.emit_add_flag_const_const_unsigned(inst, output, input_a.value(), input_b.value(), input_a.size())
            }
            (JIRValue::Slice(input_a), JIRValue::Const(input_b))
            | (JIRValue::Const(input_b), JIRValue::Slice(input_a)) => {
                // addition is commutative, hence we can just swap the const and slice arguments.
                self.emit_add_flag_slice_const(inst, output, input_a, input_b, FlagType::Carry)
            }
            (JIRValue::Slice(input_a), JIRValue::Slice(input_b)) => {
                self.emit_add_flag_slice_slice(inst, output, input_a, input_b, FlagType::Carry)
            }
        }
    }

    fn emit_add_flag_const_const_unsigned(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: u128,
        input_b: u128,
        size: JIRSize,
    ) -> JITResult {
        let (val, mut overflow) = input_a.overflowing_add(input_b);
        // Overflow is only set if 128 bit addition overflows.
        // We now check whether the result is larger than the allowed value.
        // If that is the case, an overflow occurred for the smaller const.
        let max = (1 << (size * 8)) - 1;
        if val > max {
            overflow = true;
        }
        self.emit_copy(inst, output, JIRConst::new(overflow as u128, 1))
    }
    fn emit_add_flag_const_const_signed(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: i128,
        input_b: i128,
        size: JIRSize,
    ) -> JITResult {
        let (val, mut overflow) = input_a.overflowing_add(input_b);
        // Overflow is only set if 128 bit addition overflows.
        // We now check whether the result is larger than the allowed value.
        // If that is the case, an overflow occurred for the smaller const.
        let max = (1 << (size * 8 - 1)) - 1;
        let min = !max;
        if input_a > 0 && input_b > 0 && val > max {
            overflow = true;
        }
        if input_a < 0 && input_b < 0 && val < min {
            overflow = true;
        }
        self.emit_copy(inst, output, JIRConst::new(overflow as u128, 1))
    }

    fn emit_add_flag_slice_const(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRSlice,
        input_b: JIRConst,
        flag_type: FlagType,
    ) -> JITResult {
        let Allocation::Register(output_reg) = self.allocations.get_allocation(inst, 0) else {
            todo!()
        };
        let Allocation::Register(input_a_reg) = self.allocations.get_allocation(inst, 1) else {
            todo!()
        };
        // Move input_a to output so we can modify it. Output is later overridden by the actual flag.
        self.copy_reg(output_reg, input_a_reg, input_a.size(), CopyExtendMode::None)?;
        match input_a.size() {
            1 => assemble!(self.block.assembler();
                ; add Rb(output_reg.hw_enc()), input_b.value() as i8
            ),
            2 => assemble!(self.block.assembler();
                ; add Rw(output_reg.hw_enc()), input_b.value() as i16
            ),
            4 => assemble!(self.block.assembler();
                ; add Rd(output_reg.hw_enc()), input_b.value() as i32
            ),
            8 => {
                // There are no 64 bit immediates for add, hence we need to load the immediate into a temp first.
                let temp_reg = self
                    .allocations
                    .get_temp_allocations(inst)
                    .get_nth_from_class(0, RegClass::Int)
                    .expect("We requested this");
                self.load_const(temp_reg, input_b)?;
                assemble!(self.block.assembler();
                    ; add Rq(output_reg.hw_enc()), Rq(temp_reg.hw_enc())
                );
            }
            _ => todo!("Unsupported size"),
        }
        self.emit_set_flag(output_reg, flag_type)
    }

    fn emit_add_flag_slice_slice(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRSlice,
        input_b: JIRSlice,
        flag_type: FlagType,
    ) -> JITResult {
        assert_eq!(output.size(), 1);
        assert_eq!(input_a.size(), input_b.size());
        let Allocation::Register(output_reg) = self.allocations.get_allocation(inst, 0) else {
            todo!()
        };
        let Allocation::Register(input_a_reg) = self.allocations.get_allocation(inst, 1) else {
            todo!()
        };
        let Allocation::Register(input_b_reg) = self.allocations.get_allocation(inst, 2) else {
            todo!()
        };
        self.copy_reg(output_reg, input_a_reg, input_a.size(), CopyExtendMode::None)?;
        match input_a.size() {
            1 => assemble!(self.block.assembler();
                ; add Rb(output_reg.hw_enc()), Rb(input_b_reg.hw_enc())
            ),
            2 => assemble!(self.block.assembler();
                ; add Rw(output_reg.hw_enc()), Rw(input_b_reg.hw_enc())
            ),
            4 => assemble!(self.block.assembler();
                ; add Rd(output_reg.hw_enc()), Rd(input_b_reg.hw_enc())
            ),
            8 => assemble!(self.block.assembler();
                ; add Rq(output_reg.hw_enc()), Rq(input_b_reg.hw_enc())
            ),
            _ => todo!("Unsupported size"),
        }
        self.emit_set_flag(output_reg, flag_type)
    }

    fn emit_sub_flag_value_slice(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRSlice,
        flag_type: FlagType,
    ) -> JITResult {
        let Allocation::Register(output_reg) = self.allocations.get_allocation(inst, 0) else {
            todo!()
        };
        let (input_a_reg, input_b_op_id) = match input_a {
            JIRValue::Const(input_a) => {
                // If the first argument is const, we load it into the output reg, since we will write
                // to that later anyway. Thus, we won't need an extra temporary register.
                self.load_const(output_reg, input_a)?;
                (output_reg, 1)
            }
            JIRValue::Slice(input_a) => {
                let Allocation::Register(input_a_reg) = self.allocations.get_allocation(inst, 1) else {
                    todo!()
                };
                (input_a_reg, 2)
            }
        };
        let Allocation::Register(input_b_reg) = self.allocations.get_allocation(inst, input_b_op_id) else {
            todo!()
        };

        match input_a.size() {
            1 => assemble!(self.block.assembler();
                ; cmp Rb(input_a_reg.hw_enc()), Rb(input_b_reg.hw_enc())
            ),
            2 => assemble!(self.block.assembler();
                ; cmp Rw(input_a_reg.hw_enc()), Rw(input_b_reg.hw_enc())
            ),
            4 => assemble!(self.block.assembler();
                ; cmp Rd(input_a_reg.hw_enc()), Rd(input_b_reg.hw_enc())
            ),
            8 => assemble!(self.block.assembler();
                ; cmp Rq(input_a_reg.hw_enc()), Rq(input_b_reg.hw_enc())
            ),
            _ => todo!("Unsupported size"),
        }
        self.emit_set_flag(output_reg, flag_type)
    }

    fn emit_sub_flag_slice_const(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRSlice,
        input_b: JIRConst,
        flag_type: FlagType,
    ) -> JITResult {
        let Allocation::Register(output_reg) = self.allocations.get_allocation(inst, 0) else {
            todo!()
        };
        let Allocation::Register(input_a_reg) = self.allocations.get_allocation(inst, 1) else {
            todo!()
        };
        match input_a.size() {
            1 => assemble!(self.block.assembler();
                ; cmp Rb(input_a_reg.hw_enc()), input_b.value() as i8
            ),
            2 => assemble!(self.block.assembler();
                ; cmp Rw(input_a_reg.hw_enc()), input_b.value() as i16
            ),
            4 => assemble!(self.block.assembler();
                ; cmp Rd(input_a_reg.hw_enc()), input_b.value() as i32
            ),
            8 => {
                // There are no 64 bit immediates for cmp, hence we need to load the immediate into output first.
                // Output is later overridden by the appropriate flag.
                self.load_const(output_reg, input_b)?;
                assemble!(self.block.assembler();
                    ; cmp Rq(input_a_reg.hw_enc()), Rq(output_reg.hw_enc())
                );
            }
            _ => todo!("Unsupported size"),
        }
        self.emit_set_flag(output_reg, flag_type)
    }

    fn emit_sub_flag_const_const_signed(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: i128,
        input_b: i128,
        size: JIRSize,
    ) -> JITResult {
        let (val, mut overflow) = input_a.overflowing_sub(input_b);
        // Overflow is only set if 128 bit addition overflows.
        // We now check whether the result is larger than the allowed value.
        // If that is the case, an overflow occurred for the smaller const.
        let max = (1 << (size * 8 - 1)) - 1;
        let min = !max;
        if input_a >= 0 && input_b < 0 && val > max {
            overflow = true;
        }
        if input_a <= 0 && input_b > 0 && val < min {
            overflow = true;
        }
        self.emit_copy(inst, output, JIRConst::new(overflow as u128, 1))
    }

    fn emit_set_flag(
        &mut self,
        flag_reg: HostReg,
        flag_type: FlagType,
    ) -> JITResult {
        debug_assert_eq!(flag_reg.class(), RegClass::Int);
        match flag_type {
            FlagType::Overflow => assemble!(self.block.assembler();
                ; seto Rb(flag_reg.hw_enc())
            ),
            FlagType::Carry => assemble!(self.block.assembler();
                ; setc Rb(flag_reg.hw_enc())
            ),
            FlagType::Zero => assemble!(self.block.assembler();
                ; setz Rb(flag_reg.hw_enc())
            ),
        }
        Ok(())
    }
}
