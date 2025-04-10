use crate::jit::regalloc::requirements::{
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

enum FloatOp {
    Add,
    Sub,
    Mul,
    Div,
}

impl X64AllocRequirements {
    pub fn fdiv_requirements(
        requirement: &mut AllocRequirement,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) {
        Self::fop_requirements(requirement, output, input_a, input_b);
    }

    pub fn fmul_requirements(
        requirement: &mut AllocRequirement,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) {
        Self::fop_requirements(requirement, output, input_a, input_b);
    }

    pub fn fadd_requirements(
        requirement: &mut AllocRequirement,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) {
        Self::fop_requirements(requirement, output, input_a, input_b);
    }

    pub fn fsub_requirements(
        requirement: &mut AllocRequirement,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) {
        Self::fop_requirements(requirement, output, input_a, input_b);
    }

    fn fop_requirements(
        requirement: &mut AllocRequirement,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) {
        requirement.add_request(OperandRequest::vec_write_any(output));
        match (input_a, input_b) {
            (JIRValue::Const(input_a), JIRValue::Const(input_b)) => {
                todo!("Handle float const");
            }
            (JIRValue::Slice(input_a), JIRValue::Const(input_b))
            | (JIRValue::Const(input_b), JIRValue::Slice(input_a)) => {
                todo!("Handle float const");
            }
            (JIRValue::Slice(input_a), JIRValue::Slice(input_b)) => {
                requirement.add_request(OperandRequest::vec_read_any(input_a));
                requirement.add_request(OperandRequest::vec_read_any(input_b));
            }
        }
    }
}

impl X64Emitter<'_> {
    /// Emit instructions that evaluate `a` f+ `b` and store the result in `output`.This function
    /// does not convert but instead interprets the bits of constants as floats.
    #[inline]
    pub fn emit_fadd(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        self.emit_fop(inst, output, input_a, input_b, FloatOp::Add)
    }

    /// Emit instructions that evaluate `a` f- `b` and store the result in `output`.This function
    /// does not convert but instead interprets the bits of constants as floats.
    #[inline]
    pub fn emit_fsub(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        self.emit_fop(inst, output, input_a, input_b, FloatOp::Sub)
    }

    /// Emit instructions that evaluate `a` f* `b` and store the result in `output`.This function
    /// does not convert but instead interprets the bits of constants as floats.
    #[inline]
    pub fn emit_fmul(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        self.emit_fop(inst, output, input_a, input_b, FloatOp::Mul)
    }

    /// Emit instructions that evaluate `a` f/ `b` and store the result in `output`.This function
    /// does not convert but instead interprets the bits of constants as floats.
    #[inline]
    pub fn emit_fdiv(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        self.emit_fop(inst, output, input_a, input_b, FloatOp::Div)
    }

    fn emit_fop(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
        op: FloatOp,
    ) -> JITResult {
        debug_assert_eq!(output.size(), input_a.size());
        debug_assert_eq!(input_a.size(), input_b.size());

        match (input_a, input_b) {
            (JIRValue::Const(a), JIRValue::Const(b)) => {
                todo!("implement const fops");
                // self.emit_copy(inst, output, add_constants(a, b, output.size()))?;
            }
            (JIRValue::Const(b), JIRValue::Slice(a)) | (JIRValue::Slice(a), JIRValue::Const(b)) => {
                todo!("implement const fops");
                self.emit_fop_reg_const(inst, output, a, b, op)?;
            }
            (JIRValue::Slice(a), JIRValue::Slice(b)) => {
                self.emit_fop_reg_reg(inst, output, a, b, op)?;
            }
        }

        Ok(())
    }

    fn emit_fop_reg_const(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRSlice,
        input_b: JIRConst,
        op: FloatOp,
    ) -> JITResult {
        let Allocation::Register(output_reg) = self.allocations.get_allocation(inst, 0) else {
            todo!()
        };

        self.load_const(output_reg, input_b)?;

        match input_a.size() {
            4 => {
                // single-precision
            }
            8 => {
                // double-precision
            }
            size => {
                todo!("Unsupported size {size}")
            }
        }

        Ok(())
    }

    fn emit_fop_reg_reg(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRSlice,
        input_b: JIRSlice,
        op: FloatOp,
    ) -> JITResult {
        let size = output.size();

        let Allocation::Register(output_reg) = self.allocations.get_allocation(inst, 0) else {
            todo!()
        };

        let Allocation::Register(input_a_reg) = self.allocations.get_allocation(inst, 1) else {
            todo!()
        };

        let Allocation::Register(input_b_reg) = self.allocations.get_allocation(inst, 2) else {
            todo!()
        };

        self.copy_reg(output_reg, input_a_reg, input_a.size(), CopyExtendMode::None);

        match size {
            4 => match op {
                // single-precision
                FloatOp::Add => {
                    assemble!(self.block.assembler();
                        ; addss Rx(output_reg.hw_enc()), Rx(input_b_reg.hw_enc())
                    );
                }
                FloatOp::Sub => {
                    assemble!(self.block.assembler();
                        ; subss Rx(output_reg.hw_enc()), Rx(input_b_reg.hw_enc())
                    );
                }
                FloatOp::Mul => {
                    assemble!(self.block.assembler();
                        ; mulss Rx(output_reg.hw_enc()), Rx(input_b_reg.hw_enc())
                    );
                }
                FloatOp::Div => {
                    assemble!(self.block.assembler();
                        ; divss Rx(output_reg.hw_enc()), Rx(input_b_reg.hw_enc())
                    );
                }
            }
            8 => match op {
                // double-precision
                FloatOp::Add => {
                    assemble!(self.block.assembler();
                        ; addsd Rx(output_reg.hw_enc()), Rx(input_b_reg.hw_enc())
                    );
                }
                FloatOp::Sub => {
                    assemble!(self.block.assembler();
                        ; subsd Rx(output_reg.hw_enc()), Rx(input_b_reg.hw_enc())
                    );
                }
                FloatOp::Mul => {
                    assemble!(self.block.assembler();
                        ; mulsd Rx(output_reg.hw_enc()), Rx(input_b_reg.hw_enc())
                    );
                }
                FloatOp::Div => {
                    assemble!(self.block.assembler();
                        ; divsd Rx(output_reg.hw_enc()), Rx(input_b_reg.hw_enc())
                    );
                }
            }
            size => todo!("Unsupported size {size}")
        }

        Ok(())
    }
}

fn add_constants(
    a: JIRConst,
    b: JIRConst,
    size: JIRSize,
) -> JIRConst {
    let size = a.size();
    let v = match size {
        4 => u128::from(
            (f32::from_bits(a.value().try_into().unwrap()) + f32::from_bits(b.value().try_into().unwrap())).to_bits(),
        ),
        8 => u128::from(
            (f64::from_bits(a.value().try_into().unwrap()) + f64::from_bits(b.value().try_into().unwrap())).to_bits(),
        ),
        _ => todo!("Unsupported size of {size}"),
    };
    JIRConst::new(v, size)
}
