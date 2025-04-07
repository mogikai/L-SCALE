use crate::jit::api::{
    JITResult,
    JitError,
};
use crate::jit::host::register::{
    HostReg,
    HostRegSet,
    RegClass,
};
use crate::jit::regalloc::requirements::{
    OperandRequest,
    TempRequest,
};
use crate::jit::regalloc::result::Allocation;
use crate::jit::x86::alloc_requirement::AllocRequirement;
use crate::jit::x86::assemble;
use crate::jit::x86::registers::general_purpose::GPRegister;
use crate::jit::x86::x64emitter::helpers::reg;
use crate::jit::x86::x64emitter::regalloc::SliceType;
use crate::jit::x86::x64emitter::slice_load::CopyExtendMode;
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
use std::cmp::min;

impl X64AllocRequirements {
    pub fn multiply_requirements(
        requirement: &mut AllocRequirement,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) {
        match (input_a, input_b) {
            (JIRValue::Const(input_a), JIRValue::Const(input_b)) => {
                Self::copy_requirements(requirement, output, const_multiply(input_a, input_b))
            }
            (JIRValue::Slice(input_a), JIRValue::Const(input_b))
            | (JIRValue::Const(input_b), JIRValue::Slice(input_a)) => {
                if input_b.value() == 0 {
                    Self::copy_requirements(requirement, output, input_b)
                } else {
                    Self::require_large_int_write_any(requirement, output);
                    Self::require_large_int_read_any(requirement, input_a);
                    if output.size() > 8 {
                        // We need two additional temp for the 16 byte case to load both arguments.
                        *requirement.required_temps_mut() = TempRequest::new().int(2);
                    }
                }
            }
            (JIRValue::Slice(input_a), JIRValue::Slice(input_b)) => {
                Self::require_large_int_write_fixed(
                    requirement,
                    output,
                    GPRegister::AX.into_host_reg(),
                    GPRegister::DX.into_host_reg(),
                );
                Self::require_large_int_read_any(requirement, input_a);
                Self::require_large_int_read_any(requirement, input_b);
                if output.size() > 8 {
                    // We need one additional temp for the 16 byte case.
                    *requirement.required_temps_mut() = TempRequest::new().int(1);
                }
            }
        }
    }
}

impl X64Emitter<'_> {
    #[inline]
    pub fn emit_multiply(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        debug_assert_eq!(output.size(), input_a.size());
        debug_assert_eq!(input_a.size(), input_b.size());
        match (input_a, input_b) {
            (JIRValue::Const(input_a), JIRValue::Const(input_b)) => {
                self.emit_multiply_const_const(inst, output, input_a, input_b)?;
            }
            (JIRValue::Slice(input_a), JIRValue::Const(input_b))
            | (JIRValue::Const(input_b), JIRValue::Slice(input_a)) => {
                // Since multiplication is commutative, we can just swap the slice and const arguments
                self.emit_multiply_slice_const(inst, output, input_a, input_b)?;
            }
            (JIRValue::Slice(input_a), JIRValue::Slice(input_b)) => {
                self.emit_multiply_slice_slice(inst, output, input_a, input_b)?;
            }
        }
        Ok(())
    }

    #[inline]
    fn emit_multiply_const_const(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRConst,
        input_b: JIRConst,
    ) -> Result<(), JitError> {
        debug_assert_eq!(output.size(), input_a.size());
        debug_assert_eq!(input_a.size(), input_b.size());
        self.emit_copy(inst, output, const_multiply(input_a, input_b))?;
        Ok(())
    }

    #[inline]
    fn emit_multiply_slice_const(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRSlice,
        input_b: JIRConst,
    ) -> JITResult {
        if output.size() > 8 {
            self.emit_multiply_slice_const_large(inst, output, input_a, input_b)?;
        }
        if input_b.value() == 0 {
            // We can just zero the target, since multiplying by zero always results in zero.
            return self.emit_copy(inst, output, input_b);
        }

        let output_reg = reg!(self.allocations, inst, 0);
        let input_a_reg = reg!(self.allocations, inst, 1);
        // The size in bytes needed for the immediate
        let imm_size = immediate_size(&input_b);
        match imm_size {
            1 => {
                // We can encode the constant as immediate if it fits into at most 4  bytes.
                let immediate = input_b.value();
                assemble!(self.block.assembler();
                    ; imul Rq(output_reg.hw_enc()), Rq(input_a_reg.hw_enc()), BYTE immediate as _
                );
            }
            2 => {
                // We can encode the constant as immediate if it fits into at most 4  bytes.
                let immediate = input_b.value();
                assemble!(self.block.assembler();
                    ; imul Rq(output_reg.hw_enc()), Rq(input_a_reg.hw_enc()), WORD immediate as _
                );
            }
            3..=4 => {
                // We can encode the constant as immediate if it fits into at most 4  bytes.
                let immediate = input_b.value();
                assemble!(self.block.assembler();
                    ; imul Rq(output_reg.hw_enc()), Rq(input_a_reg.hw_enc()), DWORD immediate as _
                );
            }
            5..=8 => {
                // We need to copy the input to the output, since imul reg, reg uses the first argument as
                // input and output argument.
                let temp = self.allocations.scratch_register(RegClass::Int);
                self.load_const(temp, input_b)?;
                if output_reg != input_a_reg {
                    // TODO: Can we task the reg allocator to put the input into the output beforehand?
                    self.copy_reg(output_reg, input_a_reg, output.size(), CopyExtendMode::None)?;
                }
                assemble!(self.block.assembler();
                    ; imul Rq(output_reg.hw_enc()), Rq(temp.hw_enc())
                );
            }
            _ => {
                todo!("Currently unsupported size for multiply: {}", input_b.size())
            }
        }
        Ok(())
    }

    fn emit_multiply_slice_const_large(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRSlice,
        input_b: JIRConst,
    ) -> JITResult {
        debug_assert_eq!(output.size(), 16);
        debug_assert_eq!(output.size(), input_a.size());
        debug_assert_eq!(input_a.size(), input_b.size());
        let output_reg_0 = reg!(self.allocations, inst, 0);
        let output_reg_1 = reg!(self.allocations, inst, 1);
        let input_a_reg_0 = reg!(self.allocations, inst, 2);
        let input_a_reg_1 = reg!(self.allocations, inst, 3);

        let input_b_reg_0 = self
            .allocations
            .get_temp_allocations(inst)
            .get_nth_from_class(0, RegClass::Int)
            .expect("We requested this");
        let input_b_reg_1 = self
            .allocations
            .get_temp_allocations(inst)
            .get_nth_from_class(1, RegClass::Int)
            .expect("We requested this");
        let (input_b_lower, input_b_upper) = input_b.split(8);
        self.load_const(input_b_reg_0, input_b_lower)?;
        self.load_const(input_b_reg_1, input_b_upper)?;
        self.emit_multiply_reg_reg_large(
            (output_reg_0, output_reg_1),
            (input_a_reg_0, input_a_reg_1),
            (input_b_reg_0, input_b_reg_1),
        )
    }

    #[inline]
    fn emit_multiply_slice_slice(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRSlice,
        input_b: JIRSlice,
    ) -> Result<(), JitError> {
        if output.size() > 8 {
            return self.emit_multiply_slice_slice_large(inst, output, input_a, input_b);
        }
        let output_reg = reg!(self.allocations, inst, 0);
        let input_a_reg = reg!(self.allocations, inst, 1);
        let input_b_reg = reg!(self.allocations, inst, 2);

        if output_reg == input_a_reg {
            // input_a is already in output, so we just multiply by input_b
            self.reg_mul(output_reg, input_b_reg);
        } else if output_reg == input_b_reg {
            // input_b is already in output, so we just multiply it by input_a
            self.reg_mul(output_reg, input_a_reg);
        } else {
            // Copy input_a into the output and then multiply it by input_b
            self.copy_reg(output_reg, input_a_reg, output.size(), CopyExtendMode::None)?;
            self.reg_mul(output_reg, input_b_reg);
        }
        Ok(())
    }

    fn emit_multiply_slice_slice_large(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRSlice,
        input_b: JIRSlice,
    ) -> JITResult {
        debug_assert_eq!(output.size(), 16);
        debug_assert_eq!(output.size(), input_a.size());
        debug_assert_eq!(input_a.size(), input_b.size());
        let output_reg_0 = reg!(self.allocations, inst, 0);
        let output_reg_1 = reg!(self.allocations, inst, 1);
        // We want to make sure that input_a is always the input that is potentially already in the output.
        let (input_a_reg_0, input_b_reg_0) = {
            let input_a_reg_0 = reg!(self.allocations, inst, 2);
            let input_b_reg_0 = reg!(self.allocations, inst, 4);
            if input_b_reg_0 == output_reg_0 {
                (input_b_reg_0, input_a_reg_0)
            } else {
                (input_a_reg_0, input_b_reg_0)
            }
        };
        let (input_a_reg_1, input_b_reg_1) = {
            let input_a_reg_1 = reg!(self.allocations, inst, 3);
            let input_b_reg_1 = reg!(self.allocations, inst, 5);
            if input_b_reg_1 == output_reg_1 {
                (input_b_reg_1, input_a_reg_1)
            } else {
                (input_a_reg_1, input_b_reg_1)
            }
        };
        let temp = self
            .allocations
            .get_temp_allocations(inst)
            .get_nth_from_class(0, RegClass::Int)
            .expect("We requested this");
        // Make sure that input b is in registers that can be clobbered.
        self.copy_reg(temp, input_b_reg_0, 8, CopyExtendMode::None)?;
        self.emit_multiply_reg_reg_large(
            (output_reg_0, output_reg_1),
            (input_a_reg_0, input_a_reg_1),
            (temp, input_b_reg_1),
        )
    }

    /// Multiply two 16 byte values loaded in register pairs.
    /// The output and input_a may be loaded in the same register.
    /// It is forbidden that input_b is loaded in the same register as output.
    /// input_b_0 must be loaded in a register that can be clobbered/modified.
    /// Since this function uses the scratch register, it must be kept free.
    fn emit_multiply_reg_reg_large(
        &mut self,
        (output_reg_0, output_reg_1): (HostReg, HostReg),
        (input_a_reg_0, input_a_reg_1): (HostReg, HostReg),
        (input_b_reg_0, input_b_reg_1): (HostReg, HostReg),
    ) -> JITResult {
        // q1:q0 = a0*b0 // need q1 and q0
        // r1:r0 = a0*b1 // only need r0
        // s1:s0 = a1*b0 // only need s0
        // t1:t0 = a1*b1 // not needed
        // result = t1:(t0+s1+r1):(s0+r0+q1):q0
        // Since we only need the lower 128 bits, the result is (s0+r0+q1):q0
        debug_assert_ne!(input_b_reg_0, output_reg_0);
        debug_assert_ne!(input_b_reg_1, output_reg_1);
        debug_assert_eq!(output_reg_0, GPRegister::AX.into_host_reg());
        debug_assert_eq!(output_reg_1, GPRegister::DX.into_host_reg());
        self.copy_reg(output_reg_0, input_a_reg_0, 8, CopyExtendMode::None)?;
        let scratch = self.allocations.scratch_register(RegClass::Int);
        // Save a1 from the clutches of mul
        self.copy_reg(scratch, input_a_reg_1, 8, CopyExtendMode::None)?;
        assemble!(self.block.assembler();
            // a0 is loaded in RAX
            // RDX:RAX = q1:q0 = a0 * b0
            // RAX = q0! Don't touch RAX anymore
            ; mul Rq(input_b_reg_0.hw_enc())
            // s1:s0 = b0 * a1
            // We only need s0, hence we use imul
            // b0 is no longer needed here, and we can use it as output.
            ; imul Rq(input_b_reg_0.hw_enc()), Rq(scratch.hw_enc())
            // RDX = s0 + q1
            ; add Rq(output_reg_1.hw_enc()), Rq(input_b_reg_0.hw_enc())
            // r1:r0 = a0 * b1
            // We only need r0, hence we use imul
            ; imul Rq(scratch.hw_enc()), Rq(input_b_reg_1.hw_enc())
            // RDX = RDX + r0 = r0 + s0 + q1
            ; add Rq(output_reg_1.hw_enc()), Rq(scratch.hw_enc())
            // Done! RDX:RAX = (r0 + s0 + q1):q0
        );
        Ok(())
    }

    fn reg_mul(
        &mut self,
        in_out: HostReg,
        input: HostReg,
    ) {
        assemble!(self.block.assembler();
            ; imul Rq(in_out.hw_enc()), Rq(input.hw_enc())
        );
    }
}

fn const_multiply(
    a: JIRConst,
    b: JIRConst,
) -> JIRConst {
    JIRConst::new(a.value().wrapping_mul(b.value()) % (1u128 << (a.size() * 8)), a.size())
}

fn immediate_size(value: &JIRConst) -> u32 {
    min((u128::BITS - value.value().leading_zeros() + 7) / 8, value.size().into())
}
