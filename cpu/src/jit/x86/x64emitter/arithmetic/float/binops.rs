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
        Self::binary_fop_requirements(requirement, output, input_a, input_b);
    }

    pub fn fmul_requirements(
        requirement: &mut AllocRequirement,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) {
        Self::binary_fop_requirements(requirement, output, input_a, input_b);
    }

    pub fn fadd_requirements(
        requirement: &mut AllocRequirement,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) {
        Self::binary_fop_requirements(requirement, output, input_a, input_b);
    }

    pub fn fsub_requirements(
        requirement: &mut AllocRequirement,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) {
        Self::binary_fop_requirements(requirement, output, input_a, input_b);
    }

    fn binary_fop_requirements(
        requirement: &mut AllocRequirement,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) {
        requirement.add_request(OperandRequest::vec_write_any(output));
        match (input_a, input_b) {
            (JIRValue::Const(input_a), JIRValue::Const(input_b)) => {
                // no additional requirements (result can be written directly to output)
            }
            (JIRValue::Slice(input_a), JIRValue::Const(input_b))
            | (JIRValue::Const(input_b), JIRValue::Slice(input_a)) => {
                requirement.add_request(OperandRequest::vec_read_any(input_a));
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
        self.emit_binary_fop(inst, output, input_a, input_b, FloatOp::Add)
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
        self.emit_binary_fop(inst, output, input_a, input_b, FloatOp::Sub)
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
        self.emit_binary_fop(inst, output, input_a, input_b, FloatOp::Mul)
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
        self.emit_binary_fop(inst, output, input_a, input_b, FloatOp::Div)
    }

    fn emit_binary_fop(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
        op: FloatOp,
    ) -> JITResult {
        debug_assert_eq!(output.size(), input_a.size(), "Input and output must have the same size");
        debug_assert_eq!(output.size(), input_b.size(), "Input and output must have the same size");

        match (input_a, input_b) {
            (JIRValue::Const(a), JIRValue::Const(b)) => {
                let sum = add_constants(a, b, op);
                let Allocation::Register(output_reg) = self.allocations.get_allocation(inst, 0) else {
                    todo!()
                };

                self.load_const(output_reg, sum)?;
            }
            (JIRValue::Const(input_a), JIRValue::Slice(input_b)) => {
                self.emit_fop_const_reg(inst, output, input_a, input_b, op)?;
            }
            (JIRValue::Slice(input_a), JIRValue::Const(input_b)) => {
                self.emit_fop_reg_const(inst, output, input_a, input_b, op)?;
            }
            (JIRValue::Slice(input_a), JIRValue::Slice(input_b)) => {
                self.emit_fop_reg_reg(inst, output, input_a, input_b, op)?;
            }
        }

        Ok(())
    }

    /// Emit the fop for a register first operand and constant second operand by copying the first
    /// operand into the output register and loading the second operand into the first operand
    /// register.
    fn emit_fop_reg_const(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRSlice,
        input_b: JIRConst,
        op: FloatOp,
    ) -> JITResult {
        let size = output.size();

        let Allocation::Register(output_reg) = self.allocations.get_allocation(inst, 0) else {
            todo!()
        };

        let Allocation::Register(input_a_reg) = self.allocations.get_allocation(inst, 1) else {
            todo!()
        };

        self.copy_reg(output_reg, input_a_reg, size, CopyExtendMode::None)?;
        self.load_const(input_a_reg, input_b)?;
        self.emit_fop_hostregs(output_reg, input_a_reg, op, size)?;

        Ok(())
    }

    /// Emit the fop for a constant first operand and register second operand by
    /// loading the first operand into the output register.
    fn emit_fop_const_reg(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRConst,
        input_b: JIRSlice,
        op: FloatOp,
    ) -> JITResult {
        let size = output.size();

        let Allocation::Register(output_reg) = self.allocations.get_allocation(inst, 0) else {
            todo!()
        };

        let Allocation::Register(input_b_reg) = self.allocations.get_allocation(inst, 1) else {
            todo!()
        };

        self.load_const(output_reg, input_a)?;
        self.emit_fop_hostregs(output_reg, input_b_reg, op, size)?;

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

        self.copy_reg(output_reg, input_a_reg, input_a.size(), CopyExtendMode::None)?;
        self.emit_fop_hostregs(output_reg, input_b_reg, op, size)?;

        Ok(())
    }

    /// Emit the final floating-point instruction with one operand stored in `output_reg`
    /// and the other in `input_reg`.
    fn emit_fop_hostregs(
        &mut self,
        output_reg: HostReg,
        input_reg: HostReg,
        op: FloatOp,
        size: JIRSize,
    ) -> JITResult {
        match size {
            4 => match op {
                // single-precision
                FloatOp::Add => {
                    assemble!(self.block.assembler();
                        ; addss Rx(output_reg.hw_enc()), Rx(input_reg.hw_enc())
                    );
                }
                FloatOp::Sub => {
                    assemble!(self.block.assembler();
                        ; subss Rx(output_reg.hw_enc()), Rx(input_reg.hw_enc())
                    );
                }
                FloatOp::Mul => {
                    assemble!(self.block.assembler();
                        ; mulss Rx(output_reg.hw_enc()), Rx(input_reg.hw_enc())
                    );
                }
                FloatOp::Div => {
                    assemble!(self.block.assembler();
                        ; divss Rx(output_reg.hw_enc()), Rx(input_reg.hw_enc())
                    );
                }
            },
            8 => match op {
                // double-precision
                FloatOp::Add => {
                    assemble!(self.block.assembler();
                        ; addsd Rx(output_reg.hw_enc()), Rx(input_reg.hw_enc())
                    );
                }
                FloatOp::Sub => {
                    assemble!(self.block.assembler();
                        ; subsd Rx(output_reg.hw_enc()), Rx(input_reg.hw_enc())
                    );
                }
                FloatOp::Mul => {
                    assemble!(self.block.assembler();
                        ; mulsd Rx(output_reg.hw_enc()), Rx(input_reg.hw_enc())
                    );
                }
                FloatOp::Div => {
                    assemble!(self.block.assembler();
                        ; divsd Rx(output_reg.hw_enc()), Rx(input_reg.hw_enc())
                    );
                }
            },
            size => todo!("Unsupported size {size}"),
        }

        Ok(())
    }
}

fn add_constants(
    a: JIRConst,
    b: JIRConst,
    op: FloatOp,
) -> JIRConst {
    let size = a.size();

    let v = match size {
        4 => {
            let op = match op {
                FloatOp::Add => |x, y| x + y,
                FloatOp::Sub => |x, y| x - y,
                FloatOp::Mul => |x, y| x * y,
                FloatOp::Div => |x, y| x / y,
            };
            u128::from(
                (op(f32::from_bits(a.value().try_into().unwrap()), f32::from_bits(b.value().try_into().unwrap())))
                    .to_bits(),
            )
        }
        8 => {
            let op = match op {
                FloatOp::Add => |x, y| x + y,
                FloatOp::Sub => |x, y| x - y,
                FloatOp::Mul => |x, y| x * y,
                FloatOp::Div => |x, y| x / y,
            };
            u128::from(
                (op(f64::from_bits(a.value().try_into().unwrap()), f64::from_bits(b.value().try_into().unwrap())))
                    .to_bits(),
            )
        }
        _ => todo!("Unsupported size of {size}"),
    };
    JIRConst::new(v, a.size())
}
