use crate::jit::api::{
    JITResult,
    JitError,
};
use crate::jit::host::register::RegClass;
use crate::jit::regalloc::requirements::{
    OperandRequest,
    TempRequest,
};
use crate::jit::regalloc::result::Allocation;
use crate::jit::x86::alloc_requirement::AllocRequirement;
use crate::jit::x86::assemble;
use crate::jit::x86::x64emitter::arithmetic::integer::AddSub;
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
    pub fn subtract_requirements(
        requirement: &mut AllocRequirement,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) {
        match (input_a, input_b) {
            // TODO large read/write for other cases
            (JIRValue::Const(input_a), JIRValue::Const(input_b)) => {
                Self::copy_requirements(requirement, output, sub_const(input_a, input_b));
            }
            (JIRValue::Slice(input_slice), JIRValue::Const(input_const))
            | (JIRValue::Const(input_const), JIRValue::Slice(input_slice)) => {
                Self::require_large_int_write_any(requirement, output);
                Self::require_large_int_read_any(requirement, input_slice);

                // const can be encoded as an immediate for sizes <= 4,
                // so if size > 4 we use the scratch register, but nothing else
            }
            (JIRValue::Slice(input_a), JIRValue::Slice(input_b)) if input_a == input_b => {
                // a - a = 0
                Self::copy_requirements(requirement, output, JIRConst::new(0, output.size()));
            }
            (JIRValue::Slice(input_a), JIRValue::Slice(input_b)) => {
                Self::require_large_int_write_any(requirement, output);
                Self::require_large_int_read_any(requirement, input_a);
                Self::require_large_int_read_any(requirement, input_b);
            }
        }
    }
}

impl X64Emitter<'_> {
    #[inline]
    pub fn emit_subtract(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        debug_assert_eq!(output.size(), input_a.size());
        debug_assert_eq!(input_a.size(), input_b.size());
        let size = output.size();
        match (input_a, input_b) {
            (JIRValue::Const(input_a), JIRValue::Const(input_b)) => {
                self.emit_sub_const_const(inst, output, input_a, input_b)?;
            }
            (JIRValue::Slice(input_a), JIRValue::Const(input_b)) => {
                self.emit_sub_slice_const(inst, output, input_a, input_b)?;
            }
            (JIRValue::Const(input_a), JIRValue::Slice(input_b)) => {
                self.emit_sub_const_slice(inst, output, input_a, input_b)?;
            }
            (JIRValue::Slice(input_a), JIRValue::Slice(input_b)) => {
                self.emit_sub_slice_slice(inst, output, input_a, input_b)?;
            }
        }
        Ok(())
    }

    #[inline]
    fn emit_sub_slice_const(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRSlice,
        input_b: JIRConst,
    ) -> JITResult {
        let size = output.size();
        match size {
            ..=8 => {
                self.emit_sub_slice_const_64(inst, output, input_a, input_b, size)?;
            }
            16 => self.emit_sub_slice_const_128(inst, output, input_a, input_b)?,
            size => todo!("Unsupported size {size}"),
        }

        Ok(())
    }

    #[inline]
    fn emit_sub_slice_const_64(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRSlice,
        input_b: JIRConst,
        size: JIRSize,
    ) -> JITResult {
        let output_reg = reg!(self.allocations, inst, 0);
        let input_a_reg = reg!(self.allocations, inst, 1);

        match size {
            1 => {
                // input_b fits as byte displacement into LEA
                let input_b_disp: i8 = -(input_b.value() as i8);

                assemble!(self.block.assembler();
                ; lea Rq(output_reg.hw_enc()), [BYTE Rq(input_a_reg.hw_enc()) + input_b_disp]
                );
            }
            2..=4 => {
                // input_b fits as regular displacement into LEA
                let input_b_disp: i32 = -(input_b.value() as i32);

                assemble!(self.block.assembler();
                ; lea Rq(output_reg.hw_enc()), [Rq(input_a_reg.hw_enc()) + input_b_disp]
                );
            }
            5..=8 => {
                // input_b needs to be stored in a scratch register
                let scratch_reg = self.allocations.scratch_register(RegClass::Int);
                self.load_const(scratch_reg, input_b)?;

                // store input_a into output and emit sub
                self.copy_reg(output_reg, input_a_reg, size, CopyExtendMode::None);
                assemble!(self.block.assembler();
                ; sub Rq(output_reg.hw_enc()), Rq(scratch_reg.hw_enc())
                );
            }
            size => unreachable!("We checked for this previously"),
        }

        Ok(())
    }

    fn emit_sub_slice_const_128(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRSlice,
        input_b: JIRConst
    ) -> JITResult {
        let output_reg_lower = reg!(self.allocations, inst, 0);
        let output_reg_upper = reg!(self.allocations, inst, 1);

        let input_a_reg_lower = reg!(self.allocations, inst, 2);
        let input_a_reg_upper = reg!(self.allocations, inst, 3);

        // scratch reg will be used to hold the upper and lower parts of input_b
        let scratch_reg = self.allocations.scratch_register(RegClass::Int);

        self.copy_reg(output_reg_upper, input_a_reg_upper, 8, CopyExtendMode::None)?;
        self.copy_reg(output_reg_lower, input_a_reg_lower, 8, CopyExtendMode::None)?;

        let (input_b_lower, input_b_upper) = input_b.split(8);

        // load const upper into scratch reg and subtract upper halves
        self.load_const(scratch_reg, input_b_upper);
        assemble!(self.block.assembler();
            ; sub Rq(output_reg_upper.hw_enc()), Rq(scratch_reg.hw_enc())
        );

        // load const lower into scratch reg and subtract with borrow
        self.load_const(scratch_reg, input_b_lower);
        assemble!(self.block.assembler();
            ; sbb Rq(output_reg_lower.hw_enc()), Rq(scratch_reg.hw_enc())
        );

        Ok(())
    }

    #[inline]
    fn emit_sub_const_const(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRConst,
        input_b: JIRConst,
    ) -> Result<(), JitError> {
        let size = output.size();
        self.emit_copy(inst, output, sub_const(input_a, input_b))?;
        Ok(())
    }

    #[inline]
    fn emit_sub_slice_slice(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRSlice,
        input_b: JIRSlice,
    ) -> Result<(), JitError> {
        let size = output.size();

        match size {
            ..=8 => {
                self.emit_sub_slice_slice_64(inst, output, input_a, input_b, size)?;
            }
            16 => {
                self.emit_sub_slice_slice_128(inst, output, input_a, input_b, size)?;
            }
            size => todo!("Unsupported size {size}"),
        }

        Ok(())
    }

    fn emit_sub_slice_slice_64(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRSlice,
        input_b: JIRSlice,
        size: JIRSize,
    ) -> JITResult {
        if input_a == input_b {
            // A-A = 0, so we can just zero out the output.
            return self.emit_copy(inst, output, JIRConst::new(0, size));
        }
        let output_reg = reg!(self.allocations, inst, 0);
        let input_a_reg = reg!(self.allocations, inst, 1);
        let input_b_reg = reg!(self.allocations, inst, 2);

        if output_reg == input_b_reg {
            let scratch_reg = self.allocations.scratch_register(RegClass::Int);

            // Move output=input_b to a temporary register.
            self.copy_reg(scratch_reg, output_reg, output.size(), CopyExtendMode::None)?;
            // Move input_a to output
            self.copy_reg(output_reg, input_a_reg, input_a.size(), CopyExtendMode::None)?;
            // Get the output gp register.
            // Subtract input_b=output ,which is stored in the temporary register, from the output (output=input_a).
            assemble!(self.block.assembler();
                ; sub Rq(output_reg.hw_enc()), Rq(scratch_reg.hw_enc())
            );
        } else {
            // Copy input_a into output. If output == input_a we can just load output.
            if output_reg != input_a_reg {
                self.copy_reg(output_reg, input_a_reg, input_a.size(), CopyExtendMode::None)?;
            };
            // We can just subtract from output
            assemble!(self.block.assembler();
                ; sub Rq(output_reg.hw_enc()), Rq(input_b_reg.hw_enc())
            );
        }

        Ok(())
    }

    fn emit_sub_slice_slice_128(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRSlice,
        input_b: JIRSlice,
        size: JIRSize,
    ) -> JITResult {
        if input_a == input_b {
            // A-A = 0, so we can just zero out the output.
            return self.emit_copy(inst, output, JIRConst::new(0, size));
        }

        let scratch_reg = self.allocations.scratch_register(RegClass::Int);

        let output_reg_lower = reg!(self.allocations, inst, 0);
        let output_reg_upper = reg!(self.allocations, inst, 1);

        let input_a_reg_lower = reg!(self.allocations, inst, 2);
        let input_a_reg_upper = reg!(self.allocations, inst, 3);

        let input_b_reg_lower = reg!(self.allocations, inst, 4);
        let input_b_reg_upper = reg!(self.allocations, inst, 5);

        let scratch_reg = self.allocations.scratch_register(RegClass::Int);

        let minuend_reg = if output_reg_upper == input_b_reg_upper {
            self.copy_reg(scratch_reg, output_reg_upper, output.size(), CopyExtendMode::None)?;
            self.copy_reg(output_reg_upper, input_a_reg_upper, input_a.size(), CopyExtendMode::None)?;
            scratch_reg
        } else {
            self.copy_reg(output_reg_upper, input_a_reg_upper, input_a.size(), CopyExtendMode::None)?;
            input_b_reg_upper
        };

        assemble!(self.block.assembler();
            ; sub Rq(output_reg_upper.hw_enc()), Rq(minuend_reg.hw_enc())
        );

        let minuend_reg = if output_reg_lower == input_b_reg_lower {
            self.copy_reg(scratch_reg, output_reg_lower, output.size(), CopyExtendMode::None)?;
            self.copy_reg(output_reg_lower, input_a_reg_lower, input_a.size(), CopyExtendMode::None)?;
            scratch_reg
        } else {
            self.copy_reg(output_reg_lower, input_a_reg_lower, input_a.size(), CopyExtendMode::None)?;
            input_b_reg_lower
        };

        assemble!(self.block.assembler();
            ; sbb Rq(output_reg_lower.hw_enc()), Rq(minuend_reg.hw_enc())
        );

        Ok(())
    }

    /// Emit instructions to subtract the provided [JIRSlice] `input_b` from the provided [JIRConst] `input_a`.
    /// The result is stored in the [JIRSlice] `output`.
    #[inline]
    fn emit_sub_const_slice(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRConst,
        input_b: JIRSlice,
    ) -> JITResult {
        let output_reg = reg!(self.allocations, inst, 0);
        let input_b_reg = reg!(self.allocations, inst, 1);

        if output_reg != input_b_reg {
            // If the output register does not appear on the rhs, we can use it to load the constant
            self.load_const(output_reg, input_a)?;
            // and then just subtract the operand register from the constant in the output register.
            assemble!(self.block.assembler();
                ; sub Rq(output_reg.hw_enc()), Rq(input_b_reg.hw_enc())
            );
        } else {
            let temp_reg = self.allocations.scratch_register(RegClass::Int);
            // Output appears on rhs (out = const - out)
            // Move the output into a temporary register
            self.copy_reg(temp_reg, output_reg, output.size(), CopyExtendMode::None)?;
            // Load the constant into the output
            self.load_const(output_reg, input_a)?;
            // Get the output gp register.
            // Subtract input_b=output ,which is stored in the temporary register, from the output (output=const).
            assemble!(self.block.assembler();
                ; sub Rq(output_reg.hw_enc()), Rq(temp_reg.hw_enc())
            );
        }
        Ok(())
    }
}

fn sub_const(
    input_a: JIRConst,
    input_b: JIRConst,
) -> JIRConst {
    assert_eq!(input_a.size(), input_b.size());
    JIRConst::new((input_a.value().wrapping_sub(input_b.value())) % (1u128 << (input_a.size() * 8)), input_a.size())
}
