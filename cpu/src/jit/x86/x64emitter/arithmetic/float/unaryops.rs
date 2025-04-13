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
    Neg,
    Abs,
    Sqrt,
}

impl X64AllocRequirements {
    pub fn fsqrt_requirements(
        requirement: &mut AllocRequirement,
        output: JIRSlice,
        input: JIRValue,
    ) {
        Self::unary_fop_requirements(requirement, output, input);
    }

    pub fn fneg_requirements(
        requirement: &mut AllocRequirement,
        output: JIRSlice,
        input: JIRValue,
    ) {
        Self::unary_fop_requirements(requirement, output, input);
    }

    pub fn fabs_requirements(
        requirement: &mut AllocRequirement,
        output: JIRSlice,
        input: JIRValue,
    ) {
        Self::unary_fop_requirements(requirement, output, input);
    }

    fn unary_fop_requirements(
        requirement: &mut AllocRequirement,
        output: JIRSlice,
        input: JIRValue,
    ) {
        requirement.add_request(OperandRequest::vec_write_any(output));
        match input {
            JIRValue::Const(input) => {
                *requirement.required_temps_mut() = TempRequest::new().vector(1);
            }
            JIRValue::Slice(input) => {
                requirement.add_request(OperandRequest::vec_read_any(input));
            }
        }
    }
}

impl X64Emitter<'_> {
    /// Emit instructions that calculate the square root of `input` and store the
    /// result in `output`.
    #[inline]
    pub fn emit_fsqrt(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input: JIRValue,
    ) -> JITResult {
        debug_assert_eq!(output.size(), input.size(), "Input and output must have the same size");

        let Allocation::Register(output_reg) = self.allocations.get_allocation(inst, 0) else {
            todo!();
        };

        let input_reg = match input {
            JIRValue::Slice(jirslice) => {
                let Allocation::Register(input_reg) = self.allocations.get_allocation(inst, 1) else {
                    todo!();
                };

                input_reg
            }
            JIRValue::Const(jirconst) => {
                let temp_reg = self
                    .allocations
                    .get_temp_allocations(inst)
                    .get_nth_from_class(0, RegClass::Vector)
                    .expect("We allocated this above");
                self.load_const(temp_reg, jirconst)?;
                temp_reg
            }
        };

        match input.size() {
            4 => {
                // single-precision sqrt
                assemble!(self.block.assembler();
                ; sqrtss Rx(output_reg.hw_enc()), Rx(input_reg.hw_enc())
                );
            }
            8 => {
                // double-precision sqrt
                assemble!(self.block.assembler();
                ; sqrtsd Rx(output_reg.hw_enc()), Rx(input_reg.hw_enc())
                );
            }
            size => todo!("Unsupported size {size}"),
        }

        Ok(())
    }

    /// Emit instructions that calculate the negation of `input` and store the result in `output`.
    #[inline]
    pub fn emit_fneg(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input: JIRValue,
    ) -> JITResult {
        debug_assert_eq!(output.size(), input.size(), "Input and output must have the same size");

        let size = output.size();

        let Allocation::Register(output_reg) = self.allocations.get_allocation(inst, 0) else {
            todo!();
        };

        let xor_signflip: u64 = match size {
            4 => 0x80000000,
            8 => 0x8000000000000000,
            size => todo!("Unsupported size {size}"),
        };

        match input {
            JIRValue::Slice(jirslice) => {
                let Allocation::Register(input_reg) = self.allocations.get_allocation(inst, 1) else {
                    todo!();
                };

                // load sign-flip pattern into scratch register
                let scratch_vec = self.allocations.scratch_register(RegClass::Vector);
                self.load_const(scratch_vec, JIRConst::new(xor_signflip.into(), size).into())?;

                // load input into output and xor
                self.copy_reg(output_reg, input_reg, size, CopyExtendMode::None)?;

                match size {
                    4 => {
                        // single-precision xor
                        assemble!(self.block.assembler();
                        ; xorps Rx(output_reg.hw_enc()), Rx(scratch_vec.hw_enc())
                        );
                    }
                    8 => {
                        // double-precision xor
                        assemble!(self.block.assembler();
                        ; xorpd Rx(output_reg.hw_enc()), Rx(scratch_vec.hw_enc())
                        );
                    }
                    size => todo!("Unsupported size {size}"),
                }
            }
            JIRValue::Const(jirconst) => {
                // negate by flipping sign-bit and write to output
                let negated =
                    u64::try_from(jirconst.value()).expect("If this is a valid float it can't fail") ^ xor_signflip;
                self.load_const(output_reg, JIRConst::new(negated.into(), size))?;
            }
        }

        Ok(())
    }

    /// Emit instructions that calculate the absolute of `input` and store the result in `output`.
    #[inline]
    pub fn emit_fabs(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input: JIRValue,
    ) -> JITResult {
        debug_assert_eq!(output.size(), input.size(), "Input and output must have the same size");

        let size = output.size();

        let Allocation::Register(output_reg) = self.allocations.get_allocation(inst, 0) else {
            todo!();
        };

        let notsign_mask: u64 = match size {
            4 => !0x80000000,
            8 => !0x8000000000000000,
            size => todo!("Unsupported size {size}"),
        };

        match input {
            JIRValue::Slice(jirslice) => {
                let Allocation::Register(input_reg) = self.allocations.get_allocation(inst, 1) else {
                    todo!();
                };

                // load notsign-mask pattern into scratch register
                let scratch_vec = self.allocations.scratch_register(RegClass::Vector);
                self.load_const(scratch_vec, JIRConst::new(notsign_mask.into(), size).into())?;

                // load input into output and xor
                self.copy_reg(output_reg, input_reg, size, CopyExtendMode::None)?;

                match size {
                    4 => {
                        // single-precision xor
                        assemble!(self.block.assembler();
                        ; andps Rx(output_reg.hw_enc()), Rx(scratch_vec.hw_enc())
                        );
                    }
                    8 => {
                        // double-precision xor
                        assemble!(self.block.assembler();
                        ; andpd Rx(output_reg.hw_enc()), Rx(scratch_vec.hw_enc())
                        );
                    }
                    size => todo!("Unsupported size {size}"),
                }
            }
            JIRValue::Const(jirconst) => {
                // absolute by applying notsign-mask and write to output
                let absolute =
                    u64::try_from(jirconst.value()).expect("If this is a valid float it can't fail") & notsign_mask;
                self.load_const(output_reg, JIRConst::new(absolute.into(), size))?;
            }
        }

        Ok(())
    }
}
