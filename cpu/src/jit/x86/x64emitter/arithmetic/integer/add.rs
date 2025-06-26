use crate::jit::api::JITResult;
use crate::jit::code_buffer::CompiledBasicBlock;
use crate::jit::host::register::{
    HostRegSet,
    RegClass,
};
use crate::jit::regalloc::requirements::OperandRequest;
use crate::jit::regalloc::result::Allocation;
use crate::jit::x86::alloc_requirement::AllocRequirement;
use crate::jit::x86::registers::general_purpose::GPRegister;
use crate::jit::x86::x64emitter::arithmetic::integer::AddSub;
use crate::jit::x86::x64emitter::copy::CopyExtendMode;
use crate::jit::x86::x64emitter::helpers::reg;
use crate::jit::x86::x64emitter::{
    X64AllocRequirements,
    X64Emitter,
};
use crate::jit::x86::{
    X86MogiJit,
    assemble,
};
use dynasmrt::x64::X64Relocation;
use either::Either;
use mogi_sleigh::spec::jir::basic_block::InstId;
use mogi_sleigh::spec::jir::common::JIRSize;
use mogi_sleigh::spec::jir::values::{
    JIRConst,
    JIRSlice,
    JIRValue,
};
use std::iter::once;
use tracing::trace;

impl X64AllocRequirements {
    pub fn add_requirements(
        requirement: &mut AllocRequirement,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) {
        match (input_a, input_b) {
            (JIRValue::Slice(input_a), JIRValue::Slice(input_b)) => {
                Self::require_large_int_write_any(requirement, output);
                Self::require_large_int_read_any(requirement, input_a);
                Self::require_large_int_read_any(requirement, input_b);
            }
            (JIRValue::Slice(input_a), JIRValue::Const(input_b))
            | (JIRValue::Const(input_b), JIRValue::Slice(input_a)) => {
                Self::require_large_int_write_any(requirement, output);
                Self::require_large_int_read_any(requirement, input_a);

                // const can be encoded as an immediate for sizes <= 4,
                // so if size > 4 we use the scratch register, but nothing else
            }
            (JIRValue::Const(input_a), JIRValue::Const(input_b)) => {
                // Delegate to copy
                Self::copy_requirements(requirement, output, add_constants(input_a, input_b))
            }
        }
    }
}

impl X64Emitter<'_> {
    /// Emit instructions that add `a` and `b` and store the result in `output`.
    /// Since JIR operations do not have side effects, most of the time this will
    /// result in some kind of lea operation.
    #[inline]
    pub fn emit_add(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        a: JIRValue,
        b: JIRValue,
    ) -> JITResult {
        debug_assert_eq!(output.size(), a.size());
        debug_assert_eq!(a.size(), b.size());
        match (a, b) {
            (JIRValue::Const(a), JIRValue::Const(b)) => {
                trace!("Const add optimized to move");
                self.emit_copy(inst, output, add_constants(a, b))?;
            }
            // Addition is commutative, so we can simply swap a and b if one is const and the other is a varnode.
            (JIRValue::Const(input_b), JIRValue::Slice(input_a))
            | (JIRValue::Slice(input_a), JIRValue::Const(input_b)) => {
                self.emit_add_slice_const(inst, output, input_a, input_b)?;
            }
            (JIRValue::Slice(a), JIRValue::Slice(b)) => {
                let output = reg!(self.allocations, inst, 0);
                let input_a = reg!(self.allocations, inst, 1);
                let input_b = reg!(self.allocations, inst, 2);

                assemble!(self.block.assembler();
                    ; lea Rq(output.hw_enc()), [Rq(input_a.hw_enc()) + Rq(input_b.hw_enc())]
                );
            }
        }
        Ok(())
    }

    /// Emit instructions to add the provided constant value to the register `a`.
    /// Subtraction is done by adding the inverse element of the constant.
    #[inline]
    pub(super) fn emit_add_slice_const(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRSlice,
        input_b: JIRConst,
    ) -> JITResult {
        debug_assert_eq!(output.size(), input_a.size());
        debug_assert_eq!(output.size(), input_b.size());

        let size = output.size();
        match size {
            ..=8 => {
                self.emit_add_slice_const_64(inst, output, input_a, input_b, size)?;
            }
            16 => {
                self.emit_add_slice_const_128(inst, output, input_a, input_b)?;
            }
            size => todo!("Unsupported size {size}"),
        }

        Ok(())
    }

    fn emit_add_slice_const_64(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRSlice,
        input_b: JIRConst,
        size: JIRSize,
    ) -> JITResult {
        let output_reg = reg!(self.allocations, inst, 0);
        let input_a_reg = reg!(self.allocations, inst, 1);

        const MAX_DISP_SIZE_1: u128 = i8::MAX as u128;
        const MAX_DISP_SIZE_4: u128 = i32::MAX as u128;

        match input_b.value() {
            ..MAX_DISP_SIZE_1 => {
                // input_b fits as unsigned byte displacement into LEA
                let input_b_disp: i8 = input_b.value() as i8;

                assemble!(self.block.assembler();
                ; lea Rq(output_reg.hw_enc()), [BYTE Rq(input_a_reg.hw_enc()) + input_b_disp]
                );
            }
            MAX_DISP_SIZE_1..MAX_DISP_SIZE_4 => {
                // input_b fits as regular displacement into LEA
                let input_b_disp: i32 = input_b.value() as i32;

                assemble!(self.block.assembler();
                ; lea Rq(output_reg.hw_enc()), [Rq(input_a_reg.hw_enc()) + input_b_disp]
                );
            }
            MAX_DISP_SIZE_4.. => {
                // input_b needs to be stored in a scratch register
                let scratch_reg = self.allocations.scratch_register(RegClass::Int);
                self.load_const(scratch_reg, input_b)?;

                // store input_a into output and emit add
                self.copy_reg(output_reg, input_a_reg, size, CopyExtendMode::None)?;
                assemble!(self.block.assembler();
                ; add Rq(output_reg.hw_enc()), Rq(scratch_reg.hw_enc())
                );
            }
        }

        Ok(())
    }

    fn emit_add_slice_const_128(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRSlice,
        input_b: JIRConst,
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

        // load const lower into scratch reg and add lower halves
        self.load_const(scratch_reg, input_b_lower)?;
        assemble!(self.block.assembler();
            ; add Rq(output_reg_lower.hw_enc()), Rq(scratch_reg.hw_enc())
        );

        // load const upper into scratch reg and add with carry
        self.load_const(scratch_reg, input_b_upper)?;
        assemble!(self.block.assembler();
            ; adc Rq(output_reg_upper.hw_enc()), Rq(scratch_reg.hw_enc())
        );

        Ok(())
    }
}

fn add_constants(
    a: JIRConst,
    b: JIRConst,
) -> JIRConst {
    let size = a.size();
    let a = a.value();
    let b = b.value();
    JIRConst::new((a.wrapping_add(b)) % (1u128 << (size * 8)), size)
}
