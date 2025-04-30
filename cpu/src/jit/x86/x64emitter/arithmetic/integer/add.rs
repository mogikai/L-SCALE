use crate::jit::api::JITResult;
use crate::jit::code_buffer::CompiledBasicBlock;
use crate::jit::host::register::HostRegSet;
use crate::jit::regalloc::requirements::OperandRequest;
use crate::jit::regalloc::result::Allocation;
use crate::jit::x86::alloc_requirement::AllocRequirement;
use crate::jit::x86::registers::general_purpose::GPRegister;
use crate::jit::x86::x64emitter::arithmetic::integer::AddSub;
use crate::jit::x86::x64emitter::{
    X64AllocRequirements,
    X64Emitter,
    mov64,
};
use crate::jit::x86::{
    X86MogiJit,
    assemble,
};
use dynasmrt::x64::X64Relocation;
use either::Either;
use mogi_sleigh::spec::jir::basic_block::InstId;
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
            (JIRValue::Slice(a), JIRValue::Slice(b)) => {
                requirement.add_request(OperandRequest::int_write_any(output));
                requirement.add_request(OperandRequest::int_read_any(a));
                requirement.add_request(OperandRequest::int_read_any(b));
            }
            (JIRValue::Slice(a), JIRValue::Const(b)) | (JIRValue::Const(b), JIRValue::Slice(a)) => {
                Self::add_slice_const_requirements(requirement, output, a, b);
            }
            (JIRValue::Const(a), JIRValue::Const(b)) => {
                // Delegate to copy
                Self::copy_requirements(requirement, output, add_constants(a, b))
            }
        }
    }

    pub fn add_slice_const_requirements(
        requirement: &mut AllocRequirement,
        output: JIRSlice,
        input_a: JIRSlice,
        input_b: JIRConst,
    ) {
        // If one of the operands is const, we only need one register.
        // The other value will be used as immediate.
        requirement.add_request(OperandRequest::int_write_any(output));
        requirement.add_request(OperandRequest::int_read_any(input_a));
        *requirement.clobbers_mut() = HostRegSet::empty().with(GPRegister::AX.into_host_reg())
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
            (JIRValue::Const(b), JIRValue::Slice(a)) | (JIRValue::Slice(a), JIRValue::Const(b)) => {
                self.emit_add_reg_const(inst, output, a, b.value())?;
            }
            (JIRValue::Slice(a), JIRValue::Slice(b)) => {
                let Allocation::Register(output) = self.allocations.get_allocation(inst, 0) else {
                    todo!()
                };
                let Allocation::Register(a) = self.allocations.get_allocation(inst, 1) else {
                    todo!()
                };
                let Allocation::Register(b) = self.allocations.get_allocation(inst, 2) else {
                    todo!()
                };
                assemble!(self.block.assembler();
                    ; lea Rq(output.hw_enc()), [Rq(a.hw_enc()) + Rq(b.hw_enc())]
                );
            }
        }
        Ok(())
    }

    #[inline]
    fn emit_add_reg_const(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        a: JIRSlice,
        b: u128,
    ) -> JITResult {
        self.emit_add_sub_slice_const(inst, output, a, b, AddSub::Add)
    }

    /// Emit instructions to add or subtract the provided constant value to the register `a`.
    /// Subtraction is done by adding the inverse element of the constant.
    #[inline]
    pub(super) fn emit_add_sub_slice_const(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        a: JIRSlice,
        b: u128,
        add_sub: AddSub,
    ) -> JITResult {
        let size = output.size();
        let Allocation::Register(output) = self.allocations.get_allocation(inst, 0) else {
            todo!()
        };
        let Allocation::Register(a) = self.allocations.get_allocation(inst, 1) else {
            todo!()
        };
        let output = output.hw_enc();
        let a = a.hw_enc();

        if b < i8::MAX as u128 {
            // Use byte sized displacement for operands that fit into one byte.
            let mut b = b as i8;
            if let AddSub::Sub = add_sub {
                b = -b;
            }
            assemble!(self.block.assembler();
                ; lea Rq(output), [BYTE Rq(a) + b]
            );
        } else if b < i32::MAX as u128 {
            // Use the normal 32 bit displacement for anything that fits into an i32
            let mut b = b as i32;
            if let AddSub::Sub = add_sub {
                b = -b;
            }
            assemble!(self.block.assembler();
                ; lea Rq(output), [Rq(a) + b]
            );
        } else if b < u64::MAX as u128 && size <= 8 {
            // If b is > i32::MAX, it is not possible to encode the operand as displacement for
            // a lea instruction, since the displacement can be at most 32 bits.

            // Use mov to load the 64 bit immediate into a temp register, since it is
            // the only instruction that supports a 64 bit immediate value.
            let mut b = b as i128;
            if let AddSub::Sub = add_sub {
                b = -b;
            }
            assemble!(self.block.assembler();
                ;; mov64!(self.block.assembler(); output, a)
                ; mov rax, QWORD b as _
                ; add Rq(output), rax
            );
        } else {
            todo!("Immediates > u64::MAX currently unsupported")
        }
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
