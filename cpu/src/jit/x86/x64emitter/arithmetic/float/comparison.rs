use crate::jit::regalloc::requirements::{
    self,
    OperandRequest,
    TempRequest,
};
use crate::jit::x86::alloc_requirement::AllocRequirement;
use crate::jit::x86::assemble;
use crate::jit::x86::x64emitter::copy::CopyExtendMode;
use mogi_sleigh::spec::jir::basic_block::InstId;
use mogi_sleigh::spec::jir::common::JIRSize;
use mogi_sleigh::spec::jir::values::{
    JIRConst,
    JIRSlice,
    JIRValue,
};
use tracing::trace;

use crate::jit::api::JITResult;
use crate::jit::host::register::{
    HostReg,
    RegClass,
};
use crate::jit::regalloc::result::Allocation;
use crate::jit::x86::x64emitter::{
    X64AllocRequirements,
    X64Emitter,
};

enum FloatComp {
    Eq,
    NotEq,
    Less,
    LessEq,
    Greater,
    GreaterEq,
}

impl X64AllocRequirements {
    pub fn fless_requirements(
        requirements: &mut AllocRequirement,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) {
        Self::fcomp_requirements(requirements, output, input_a, input_b);
    }

    pub fn flesseq_requirements(
        requirements: &mut AllocRequirement,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) {
        Self::fcomp_requirements(requirements, output, input_a, input_b);
    }

    pub fn fgreater_requirements(
        requirements: &mut AllocRequirement,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) {
        Self::fcomp_requirements(requirements, output, input_a, input_b);
    }

    pub fn fgreatereq_requirements(
        requirements: &mut AllocRequirement,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) {
        Self::fcomp_requirements(requirements, output, input_a, input_b);
    }

    pub fn feq_requirements(
        requirements: &mut AllocRequirement,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) {
        Self::fcomp_requirements(requirements, output, input_a, input_b);
    }

    pub fn fneq_requirements(
        requirements: &mut AllocRequirement,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) {
        Self::fcomp_requirements(requirements, output, input_a, input_b);
    }

    fn fcomp_requirements(
        requirements: &mut AllocRequirement,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) {
        requirements.add_request(OperandRequest::int_write_any(output));

        match (input_a, input_b) {
            (JIRValue::Const(_), JIRValue::Const(_)) => {
                // directly write result, no registers necessary
            }
            (JIRValue::Slice(input_a), JIRValue::Const(_)) | (JIRValue::Const(_), JIRValue::Slice(input_a)) => {
                *requirements.required_temps_mut() = TempRequest::new().vector(1);
                requirements.add_request(OperandRequest::vec_read_any(input_a));
            }
            (JIRValue::Slice(input_a), JIRValue::Slice(input_b)) => {
                requirements.add_request(OperandRequest::vec_read_any(input_a));
                requirements.add_request(OperandRequest::vec_read_any(input_b));
            }
        }
    }
}

impl X64Emitter<'_> {
    pub fn emit_fless(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        self.emit_fcomp(inst, output, input_a, input_b, FloatComp::Less)
    }

    pub fn emit_flesseq(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        self.emit_fcomp(inst, output, input_a, input_b, FloatComp::LessEq)
    }

    pub fn emit_fgreater(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        self.emit_fcomp(inst, output, input_a, input_b, FloatComp::Greater)
    }

    pub fn emit_fgreatereq(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        self.emit_fcomp(inst, output, input_a, input_b, FloatComp::GreaterEq)
    }

    pub fn emit_feq(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        self.emit_fcomp(inst, output, input_a, input_b, FloatComp::Eq)
    }

    pub fn emit_fneq(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        self.emit_fcomp(inst, output, input_a, input_b, FloatComp::NotEq)
    }

    fn emit_fcomp(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
        comp: FloatComp,
    ) -> JITResult {
        assert_eq!(input_a.size(), input_b.size(), "Output and input must have an equal size");
        assert_eq!(output.size(), 1, "Comparison results must have a size of one byte");

        let size = input_a.size();

        let Allocation::Register(output_reg) = self.allocations.get_allocation(inst, 0) else {
            todo!();
        };

        match (input_a, input_b) {
            (JIRValue::Const(input_a), JIRValue::Const(input_b)) => {
                // directly write result, no registers necessary
                let result = comp.compare(input_a, input_b, size);
                self.load_const(output_reg, result)?;
            }
            (JIRValue::Slice(input_a), JIRValue::Const(input_b)) => {
                self.emit_fcomp_reg_const(inst, output, input_a, input_b, comp)?;
            }
            (JIRValue::Const(input_a), JIRValue::Slice(input_b)) => {
                self.emit_fcomp_const_reg(inst, output, input_a, input_b, comp)?;
            }
            (JIRValue::Slice(input_a), JIRValue::Slice(input_b)) => {
                self.emit_fcomp_reg_reg(inst, output, input_a, input_b, comp)?;
            }
        }

        Ok(())
    }

    fn emit_fcomp_reg_const(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRSlice,
        input_b: JIRConst,
        comp: FloatComp,
    ) -> JITResult {
        // load input_b into temp register and emit comparison
        let temp_reg = self
            .allocations
            .get_temp_allocations(inst)
            .get_nth_from_class(0, RegClass::Vector)
            .expect("We allocated this above");
        self.load_const(temp_reg, input_b)?;

        let Allocation::Register(output_reg) = self.allocations.get_allocation(inst, 0) else {
            todo!();
        };

        let Allocation::Register(input_a_reg) = self.allocations.get_allocation(inst, 1) else {
            todo!();
        };

        self.emit_fcomp_hostregs(inst, output_reg, input_a_reg, temp_reg, comp, input_a.size())
    }

    fn emit_fcomp_const_reg(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRConst,
        input_b: JIRSlice,
        comp: FloatComp,
    ) -> JITResult {
        // load input_a into temp register and emit comparison
        let temp_reg = self
            .allocations
            .get_temp_allocations(inst)
            .get_nth_from_class(0, RegClass::Vector)
            .expect("We allocated this above");
        self.load_const(temp_reg, input_a)?;

        let Allocation::Register(output_reg) = self.allocations.get_allocation(inst, 0) else {
            todo!();
        };

        let Allocation::Register(input_b_reg) = self.allocations.get_allocation(inst, 1) else {
            todo!();
        };

        self.emit_fcomp_hostregs(inst, output_reg, temp_reg, input_b_reg, comp, input_a.size())
    }

    fn emit_fcomp_reg_reg(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRSlice,
        input_b: JIRSlice,
        comp: FloatComp,
    ) -> JITResult {
        let Allocation::Register(output_reg) = self.allocations.get_allocation(inst, 0) else {
            todo!();
        };

        let Allocation::Register(input_a_reg) = self.allocations.get_allocation(inst, 1) else {
            todo!();
        };

        let Allocation::Register(input_b_reg) = self.allocations.get_allocation(inst, 2) else {
            todo!();
        };

        self.emit_fcomp_hostregs(inst, output_reg, input_a_reg, input_b_reg, comp, input_a.size())
    }

    fn emit_fcomp_hostregs(
        &mut self,
        inst: InstId,
        output_reg: HostReg,
        input_a_reg: HostReg,
        input_b_reg: HostReg,
        comp: FloatComp,
        size: JIRSize,
    ) -> JITResult {
        match size {
            4 => {
                // single-precision float comparison
                assemble!(self.block.assembler();
                ; ucomiss Rx(input_a_reg.hw_enc()), Rx(input_b_reg.hw_enc())
                );
            }
            8 => {
                // double-precision float comparison
                assemble!(self.block.assembler();
                ; ucomisd Rx(input_a_reg.hw_enc()), Rx(input_b_reg.hw_enc())
                );
            }
            size => todo!("Unsupported size {size}"),
        }

        match comp {
            FloatComp::Eq => {
                assemble!(self.block.assembler();
                ; sete Rb(output_reg.hw_enc())
                );
            }
            FloatComp::NotEq => {
                assemble!(self.block.assembler();
                ; setne Rb(output_reg.hw_enc())
                );
            }
            FloatComp::Less => {
                assemble!(self.block.assembler();
                ; setb Rb(output_reg.hw_enc())
                );
            }
            FloatComp::LessEq => {
                assemble!(self.block.assembler();
                ; setbe Rb(output_reg.hw_enc())
                );
            }
            FloatComp::Greater => {
                assemble!(self.block.assembler();
                ; seta Rb(output_reg.hw_enc())
                );
            }
            FloatComp::GreaterEq => {
                assemble!(self.block.assembler();
                ; setae Rb(output_reg.hw_enc())
                );
            }
        }

        Ok(())
    }
}

impl FloatComp {
    fn compare(
        &self,
        input_a: JIRConst,
        input_b: JIRConst,
        size: JIRSize,
    ) -> JIRConst {
        let result = match size {
            4 => {
                // single-precision comparison
                let (input_a, input_b): (f32, f32) = (
                    f32::from_bits(u32::try_from(input_a.value()).expect("This must be a valid float")),
                    f32::from_bits(u32::try_from(input_b.value()).expect("This must be a valid float")),
                );

                match self {
                    FloatComp::Eq => input_a == input_b,
                    FloatComp::NotEq => input_a != input_b,
                    FloatComp::Less => input_a < input_b,
                    FloatComp::LessEq => input_a <= input_b,
                    FloatComp::Greater => input_a > input_b,
                    FloatComp::GreaterEq => input_a >= input_b,
                }
            }
            8 => {
                // double-precision comparison
                let (input_a, input_b): (f64, f64) = (
                    f64::from_bits(u64::try_from(input_a.value()).expect("This must be a valid float")),
                    f64::from_bits(u64::try_from(input_b.value()).expect("This must be a valid float")),
                );

                match self {
                    FloatComp::Eq => input_a == input_b,
                    FloatComp::NotEq => input_a != input_b,
                    FloatComp::Less => input_a < input_b,
                    FloatComp::LessEq => input_a <= input_b,
                    FloatComp::Greater => input_a > input_b,
                    FloatComp::GreaterEq => input_a >= input_b,
                }
            }
            size => todo!("Unsupported size {size}")
        };

        match result {
            true => JIRConst::new(1u128, 1),
            false => JIRConst::new(0u128, 1),
        }
    }
}
