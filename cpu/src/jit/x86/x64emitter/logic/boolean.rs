use crate::jit::api::JITResult;
use crate::jit::host::register::HostReg;
use crate::jit::regalloc::requirements::OperandRequest;
use crate::jit::regalloc::result::Allocation;
use crate::jit::x86::alloc_requirement::AllocRequirement;
use crate::jit::x86::assemble;
use crate::jit::x86::x64emitter::regalloc::SliceType;
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

impl X64AllocRequirements {
    pub fn boolean_requirements(
        requirement: &mut AllocRequirement,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) {
        Self::bitwise_requirements(requirement, output, input_a, input_b);
    }

    pub fn boolean_negate_requirements(
        requirement: &mut AllocRequirement,
        output: JIRSlice,
        input_a: JIRValue,
    ) {
        Self::bitwise_negate_requirements(requirement, output, input_a);
    }
}

impl X64Emitter<'_> {
    /// Emit instructions to normalize the boolean inputs
    fn emit_normalize_boolean_inputs(
        &mut self,
        inst: InstId,
        input_a: &mut JIRValue,
        input_b: &mut JIRValue,
    ) -> JITResult {
        match (input_a, input_b) {
            (JIRValue::Const(input_a), JIRValue::Const(input_b)) => {
                *input_a.value_mut() = (input_a.value() != 0) as u128;
                *input_b.value_mut() = (input_b.value() != 0) as u128;
            }
            (JIRValue::Slice(input_a), JIRValue::Const(input_b))
            | (JIRValue::Const(input_b), JIRValue::Slice(input_a)) => {
                let Allocation::Register(input_a_reg) = self.allocations.get_allocation(inst, 1) else {
                    todo!()
                };

                *input_b.value_mut() = (input_b.value() != 0) as u128;

                self.emit_normalize_boolean_input_reg(inst, input_a_reg)?;
            }
            (JIRValue::Slice(input_a), JIRValue::Slice(input_b)) => {
                let Allocation::Register(input_a_reg) = self.allocations.get_allocation(inst, 1) else {
                    todo!()
                };

                let Allocation::Register(input_b_reg) = self.allocations.get_allocation(inst, 2) else {
                    todo!()
                };

                self.emit_normalize_boolean_input_reg(inst, input_a_reg)?;
                self.emit_normalize_boolean_input_reg(inst, input_b_reg)?;
            }
        }

        Ok(())
    }

    fn emit_normalize_boolean_input(
        &mut self,
        inst: InstId,
        input: &mut JIRValue,
    ) -> JITResult {
        match input {
            JIRValue::Slice(jirslice) => {
                let Allocation::Register(input_reg) = self.allocations.get_allocation(inst, 1) else {
                    todo!()
                };

                self.emit_normalize_boolean_input_reg(inst, input_reg)?;
            }
            JIRValue::Const(input) => {
                *input.value_mut() = (input.value() != 0) as u128;
            }
        }
        Ok(())
    }

    fn emit_normalize_boolean_input_reg(
        &mut self,
        inst: InstId,
        input_reg: HostReg,
    ) -> JITResult {
        // TODO: We can emit this normalization in some cases, if we have a marker somewhere
        //       where we can save if a value has already been normalized.
        //       this is also the case for example if `input` is the result of a boolean expression.
        assemble!(self.block.assembler();
            ; cmp Rb(input_reg.hw_enc()), 0
            ; setnz Rb(input_reg.hw_enc())
        );
        Ok(())
    }

    pub fn emit_boolean_and(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        mut input_a: JIRValue,
        mut input_b: JIRValue,
    ) -> JITResult {
        if output.size() != 1 || output.size() != input_a.size() || input_a.size() != input_b.size() {
            todo!("Invalid boolean size")
        }
        self.emit_normalize_boolean_inputs(inst, &mut input_a, &mut input_b)?;
        self.emit_bitwise_and(inst, output, input_a, input_b)
    }

    pub fn emit_boolean_or(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        mut input_a: JIRValue,
        mut input_b: JIRValue,
    ) -> JITResult {
        if output.size() != 1 || output.size() != input_a.size() || input_a.size() != input_b.size() {
            todo!("Invalid boolean size")
        }
        self.emit_normalize_boolean_inputs(inst, &mut input_a, &mut input_b)?;
        self.emit_bitwise_or(inst, output, input_a, input_b)
    }

    pub fn emit_boolean_xor(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        mut input_a: JIRValue,
        mut input_b: JIRValue,
    ) -> JITResult {
        if output.size() != 1 || output.size() != input_a.size() || input_a.size() != input_b.size() {
            todo!("Invalid boolean size")
        }
        self.emit_normalize_boolean_inputs(inst, &mut input_a, &mut input_b)?;
        self.emit_bitwise_xor(inst, output, input_a, input_b)
    }

    pub fn emit_boolean_negate(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        mut input: JIRValue,
    ) -> JITResult {
        if output.size() != 1 || output.size() != input.size() {
            todo!("Invalid boolean size")
        }
        self.emit_normalize_boolean_input(inst, &mut input)?;
        self.emit_bitwise_xor(inst, output, input, JIRConst::new(1, 1).into())
    }
}
