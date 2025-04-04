use crate::jit::api::JITResult;
use crate::jit::assembler::MogiAssembler;
use crate::jit::host::register::HostReg;
use crate::jit::regalloc::requirements::OperandRequest;
use crate::jit::regalloc::result::Allocation;
use crate::jit::x86::alloc_requirement::AllocRequirement;
use crate::jit::x86::assemble;
use crate::jit::x86::registers::general_purpose::GPRegister;
use crate::jit::x86::x64emitter::regalloc::SliceType;
use crate::jit::x86::x64emitter::{
    X64AllocRequirements,
    X64Emitter,
};
use dynasmrt::x64::X64Relocation;
use mogi_sleigh::spec::jir::basic_block::InstId;
use mogi_sleigh::spec::jir::values::{
    JIRConst,
    JIRSlice,
    JIRValue,
};

impl X64AllocRequirements {
    pub fn compare_requirements(
        requirement: &mut AllocRequirement,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) {
        match (input_a, input_b) {
            (JIRValue::Const(input_a), JIRValue::Const(input_b)) => {
                // Specify pseudo value, since the input will be combined into a const with size one.
                Self::copy_requirements(requirement, output, JIRConst::new(0, 1));
            }
            (JIRValue::Slice(input_a), JIRValue::Const(input_b))
            | (JIRValue::Const(input_b), JIRValue::Slice(input_a)) => {
                requirement.add_request(OperandRequest::int_write_any(output));
                requirement.add_request(OperandRequest::int_read_any(input_a));
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
    pub fn emit_compare_eq(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        self.emit_compare_commutative(
            inst,
            output,
            input_a,
            input_b,
            // Casting bool to u128 results in 0u128 if false and 1u128 if true.
            // See: https://doc.rust-lang.org/std/primitive.bool.html
            |a, b| (a == b) as u128,
            |assembler, out_reg| {
                assemble!(assembler;
                    ; sete Rb(out_reg.hw_enc())
                );
            },
        )
    }

    pub fn emit_compare_ne(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        self.emit_compare_commutative(
            inst,
            output,
            input_a,
            input_b,
            |a, b| (a != b) as u128,
            |assembler, out_reg| {
                assemble!(assembler;
                    ; setne Rb(out_reg.hw_enc())
                );
            },
        )
    }

    pub fn emit_compare_lt(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        self.emit_compare(
            inst,
            output,
            input_a,
            input_b,
            |a, b| (a < b) as u128,
            |assembler, out_reg| {
                assemble!(assembler;
                    ; setb Rb(out_reg.hw_enc())
                )
            },
            |assembler, out_reg| {
                assemble!(assembler;
                    ; seta Rb(out_reg.hw_enc())
                )
            },
        )
    }

    pub fn emit_compare_lt_signed(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        self.emit_compare(
            inst,
            output,
            input_a,
            input_b,
            |a, b| ((a as i128) < (b as i128)) as u128,
            |assembler, out_reg| {
                assemble!(assembler;
                    ; setl Rb(out_reg.hw_enc())
                )
            },
            |assembler, out_reg| {
                assemble!(assembler;
                    ; setg Rb(out_reg.hw_enc())
                )
            },
        )
    }

    pub fn emit_compare_le(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        self.emit_compare(
            inst,
            output,
            input_a,
            input_b,
            |a, b| (a <= b) as u128,
            |assembler, out_reg| {
                assemble!(assembler;
                    ; setbe Rb(out_reg.hw_enc())
                );
            },
            |assembler, out_reg| {
                assemble!(assembler;
                    ; setae Rb(out_reg.hw_enc())
                )
            },
        )
    }

    pub fn emit_compare_le_signed(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        self.emit_compare(
            inst,
            output,
            input_a,
            input_b,
            |a, b| ((a as i128) <= (b as i128)) as u128,
            |assembler, out_reg| {
                assemble!(assembler;
                    ; setle Rb(out_reg.hw_enc())
                );
            },
            |assembler, out_reg| {
                assemble!(assembler;
                    ; setge Rb(out_reg.hw_enc())
                )
            },
        )
    }

    pub fn emit_compare_gt(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        self.emit_compare(
            inst,
            output,
            input_a,
            input_b,
            |a, b| (a > b) as u128,
            |assembler, out_reg| {
                assemble!(assembler;
                    ; seta Rb(out_reg.hw_enc())
                );
            },
            |assembler, out_reg| {
                assemble!(assembler;
                    ; setb Rb(out_reg.hw_enc())
                )
            },
        )
    }

    pub fn emit_compare_gt_signed(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        self.emit_compare(
            inst,
            output,
            input_a,
            input_b,
            |a, b| ((a as i128) > (b as i128)) as u128,
            |assembler, out_reg| {
                assemble!(assembler;
                    ; setg Rb(out_reg.hw_enc())
                );
            },
            |assembler, out_reg| {
                assemble!(assembler;
                    ; setl Rb(out_reg.hw_enc())
                )
            },
        )
    }

    pub fn emit_compare_ge(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        self.emit_compare(
            inst,
            output,
            input_a,
            input_b,
            |a, b| (a >= b) as u128,
            |assembler, out_reg| {
                assemble!(assembler;
                    ; setae Rb(out_reg.hw_enc())
                );
            },
            |assembler, out_reg| {
                assemble!(assembler;
                    ; setbe Rb(out_reg.hw_enc())
                )
            },
        )
    }

    pub fn emit_compare_ge_signed(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        self.emit_compare(
            inst,
            output,
            input_a,
            input_b,
            |a, b| ((a as i128) >= (b as i128)) as u128,
            |assembler, out_reg| {
                assemble!(assembler;
                    ; setge Rb(out_reg.hw_enc())
                );
            },
            |assembler, out_reg| {
                assemble!(assembler;
                    ; setle Rb(out_reg.hw_enc())
                )
            },
        )
    }

    fn emit_compare_commutative(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
        cmp: fn(u128, u128) -> u128,
        asm_set: fn(&mut MogiAssembler<X64Relocation>, HostReg),
    ) -> JITResult {
        assert_eq!(output.size(), 1);
        assert_eq!(input_a.size(), input_b.size());
        match (input_a, input_b) {
            (JIRValue::Const(input_a), JIRValue::Const(input_b)) => {
                self.emit_copy(inst, output, JIRConst::new(cmp(input_a.value(), input_b.value()), 1))
            }
            (JIRValue::Slice(input_a), JIRValue::Const(input_b))
            | (JIRValue::Const(input_b), JIRValue::Slice(input_a)) => {
                // We match either variant of const, slice because eq is commutative.
                self.emit_cmp_slice_const(inst, asm_set, output, input_a, input_b)
            }
            (JIRValue::Slice(input_a), JIRValue::Slice(input_b)) => {
                self.emit_cmp_slice_slice(inst, asm_set, output, input_a, input_b)
            }
        }
    }

    fn emit_compare(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
        cmp: fn(u128, u128) -> u128,
        asm_set: fn(&mut MogiAssembler<X64Relocation>, HostReg),
        asm_set_inverted: fn(&mut MogiAssembler<X64Relocation>, HostReg),
    ) -> JITResult {
        assert_eq!(output.size(), 1);
        assert_eq!(input_a.size(), input_b.size());
        match (input_a, input_b) {
            (JIRValue::Const(input_a), JIRValue::Const(input_b)) => {
                self.emit_copy_deprecated(output, JIRConst::new(cmp(input_a.value(), input_b.value()), 1))
            }
            (JIRValue::Slice(input_a), JIRValue::Const(input_b)) => {
                self.emit_cmp_slice_const(inst, asm_set, output, input_a, input_b)
            }
            (JIRValue::Const(input_a), JIRValue::Slice(input_b)) => {
                self.emit_cmp_const_slice(inst, asm_set_inverted, output, input_a, input_b)
            }
            (JIRValue::Slice(input_a), JIRValue::Slice(input_b)) => {
                self.emit_cmp_slice_slice(inst, asm_set, output, input_a, input_b)
            }
        }
    }

    fn emit_cmp_const_slice(
        &mut self,
        inst: InstId,
        asm_set_inverted: fn(&mut MogiAssembler<X64Relocation>, HostReg),
        output: JIRSlice,
        input_a: JIRConst,
        input_b: JIRSlice,
    ) -> JITResult {
        let Allocation::Register(output_reg) = self.allocations.get_allocation(inst, 0) else {
            todo!()
        };
        let Allocation::Register(input_b_reg) = self.allocations.get_allocation(inst, 1) else {
            todo!()
        };
        // We swap arguments a and b and perform the inverted comparison such that
        // we can use an immediate if input_a is const. (e.g. a < b is done by comparing b > a)
        let imm_32 = input_a.value() as i32;
        match input_a.size() {
            1 => assemble!(self.block.assembler();
                ; cmp Rb(input_b_reg.hw_enc()), imm_32 as i8
            ),
            2 => assemble!(self.block.assembler();
                ; cmp Rw(input_b_reg.hw_enc()), imm_32 as i16
            ),
            4 => assemble!(self.block.assembler();
                ; cmp Rd(input_b_reg.hw_enc()), imm_32
            ),
            8 => {
                // For 64bit constants, we need to load the constant into a register.
                self.load_const(output_reg, input_a)?;
                assemble!(self.block.assembler();
                    ; cmp Rq(input_b_reg.hw_enc()), Rq(output_reg.hw_enc())
                );
            }
            _ => todo!(),
        }
        asm_set_inverted(self.block.assembler(), output_reg);
        Ok(())
    }

    fn emit_cmp_slice_const(
        &mut self,
        inst: InstId,
        asm_set: fn(&mut MogiAssembler<X64Relocation>, HostReg),
        output: JIRSlice,
        input_a: JIRSlice,
        input_b: JIRConst,
    ) -> JITResult {
        let Allocation::Register(output_reg) = self.allocations.get_allocation(inst, 0) else {
            todo!()
        };
        let Allocation::Register(input_a_reg) = self.allocations.get_allocation(inst, 1) else {
            todo!()
        };
        let imm_32 = input_b.value() as i32;
        match input_a.size() {
            1 => assemble!(self.block.assembler();
                ; cmp Rb(input_a_reg.hw_enc()), imm_32 as i8
            ),
            2 => assemble!(self.block.assembler();
                ; cmp Rw(input_a_reg.hw_enc()), imm_32 as i16
            ),
            4 => assemble!(self.block.assembler();
                ; cmp Rd(input_a_reg.hw_enc()), imm_32
            ),
            8 => {
                // For 64 bit constants, we need to load the constant into a register.
                self.load_const(output_reg, input_b)?;
                assemble!(self.block.assembler();
                    ; cmp Rq(input_a_reg.hw_enc()), Rq(output_reg.hw_enc())
                );
            }
            _ => todo!(),
        }
        asm_set(self.block.assembler(), output_reg);
        Ok(())
    }

    fn emit_cmp_slice_slice(
        &mut self,
        inst: InstId,
        asm_set: fn(&mut MogiAssembler<X64Relocation>, HostReg),
        output: JIRSlice,
        input_a: JIRSlice,
        input_b: JIRSlice,
    ) -> JITResult {
        let Allocation::Register(output_reg) = self.allocations.get_allocation(inst, 0) else {
            todo!()
        };
        let Allocation::Register(input_a_reg) = self.allocations.get_allocation(inst, 1) else {
            todo!()
        };
        let Allocation::Register(input_b_reg) = self.allocations.get_allocation(inst, 2) else {
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
            _ => todo!(),
        }
        asm_set(self.block.assembler(), output_reg);
        Ok(())
    }
}
