use crate::jit::api::JITResult;
use crate::jit::assembler::MogiAssembler;
use crate::jit::x86::regalloc::SliceType;
use crate::jit::x86::registers::GPRegister;
use crate::jit::x86::{
    X86MogiJit,
    assemble,
};
use dynasmrt::x64::X64Relocation;
use mogi_sleigh::spec::jir::values::{
    JIRConst,
    JIRSlice,
    JIRValue,
};

impl X86MogiJit {
    pub fn emit_compare_eq(
        &mut self,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        self.emit_compare_commutative(
            output,
            input_a,
            input_b,
            // Casting bool to u128 results in 0u128 if false and 1u128 if true.
            // See: https://doc.rust-lang.org/std/primitive.bool.html
            |a, b| (a == b) as u128,
            |assembler, out_gpr| {
                assemble!(assembler;
                    ; sete Rb(out_gpr as u8)
                );
            },
        )
    }

    pub fn emit_compare_ne(
        &mut self,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        self.emit_compare_commutative(
            output,
            input_a,
            input_b,
            |a, b| (a != b) as u128,
            |assembler, out_gpr| {
                assemble!(assembler;
                    ; setne Rb(out_gpr as u8)
                );
            },
        )
    }

    pub fn emit_compare_lt(
        &mut self,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        self.emit_compare(
            output,
            input_a,
            input_b,
            |a, b| (a < b) as u128,
            |assembler, out_gpr| {
                assemble!(assembler;
                    ; setb Rb(out_gpr as u8)
                )
            },
            |assembler, out_gpr| {
                assemble!(assembler;
                    ; seta Rb(out_gpr as u8)
                )
            },
        )
    }

    pub fn emit_compare_lt_signed(
        &mut self,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        self.emit_compare(
            output,
            input_a,
            input_b,
            |a, b| ((a as i128) < (b as i128)) as u128,
            |assembler, out_gpr| {
                assemble!(assembler;
                    ; setl Rb(out_gpr as u8)
                )
            },
            |assembler, out_gpr| {
                assemble!(assembler;
                    ; setg Rb(out_gpr as u8)
                )
            },
        )
    }

    pub fn emit_compare_le(
        &mut self,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        self.emit_compare(
            output,
            input_a,
            input_b,
            |a, b| (a <= b) as u128,
            |assembler, out_gpr| {
                assemble!(assembler;
                    ; setbe Rb(out_gpr as u8)
                );
            },
            |assembler, out_gpr| {
                assemble!(assembler;
                    ; setae Rb(out_gpr as u8)
                )
            },
        )
    }

    pub fn emit_compare_le_signed(
        &mut self,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        self.emit_compare(
            output,
            input_a,
            input_b,
            |a, b| ((a as i128) <= (b as i128)) as u128,
            |assembler, out_gpr| {
                assemble!(assembler;
                    ; setle Rb(out_gpr as u8)
                );
            },
            |assembler, out_gpr| {
                assemble!(assembler;
                    ; setge Rb(out_gpr as u8)
                )
            },
        )
    }

    pub fn emit_compare_gt(
        &mut self,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        self.emit_compare(
            output,
            input_a,
            input_b,
            |a, b| (a > b) as u128,
            |assembler, out_gpr| {
                assemble!(assembler;
                    ; seta Rb(out_gpr as u8)
                );
            },
            |assembler, out_gpr| {
                assemble!(assembler;
                    ; setb Rb(out_gpr as u8)
                )
            },
        )
    }

    pub fn emit_compare_gt_signed(
        &mut self,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        self.emit_compare(
            output,
            input_a,
            input_b,
            |a, b| ((a as i128) > (b as i128)) as u128,
            |assembler, out_gpr| {
                assemble!(assembler;
                    ; setg Rb(out_gpr as u8)
                );
            },
            |assembler, out_gpr| {
                assemble!(assembler;
                    ; setl Rb(out_gpr as u8)
                )
            },
        )
    }

    pub fn emit_compare_ge(
        &mut self,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        self.emit_compare(
            output,
            input_a,
            input_b,
            |a, b| (a >= b) as u128,
            |assembler, out_gpr| {
                assemble!(assembler;
                    ; setae Rb(out_gpr as u8)
                );
            },
            |assembler, out_gpr| {
                assemble!(assembler;
                    ; setbe Rb(out_gpr as u8)
                )
            },
        )
    }

    pub fn emit_compare_ge_signed(
        &mut self,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        self.emit_compare(
            output,
            input_a,
            input_b,
            |a, b| ((a as i128) >= (b as i128)) as u128,
            |assembler, out_gpr| {
                assemble!(assembler;
                    ; setge Rb(out_gpr as u8)
                );
            },
            |assembler, out_gpr| {
                assemble!(assembler;
                    ; setle Rb(out_gpr as u8)
                )
            },
        )
    }

    fn emit_compare_commutative(
        &mut self,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
        cmp: fn(u128, u128) -> u128,
        asm_set: fn(&mut MogiAssembler<X64Relocation>, GPRegister),
    ) -> JITResult {
        assert_eq!(output.size(), 1);
        assert_eq!(input_a.size(), input_b.size());
        let out_gpr = self.allocate_slice_gpr(output, SliceType::Output);
        match (input_a, input_b) {
            (JIRValue::Const(input_a), JIRValue::Const(input_b)) => {
                self.emit_copy(output, JIRConst::new(cmp(input_a.value(), input_b.value()), 1))
            }
            (JIRValue::Slice(input_a), JIRValue::Const(input_b))
            | (JIRValue::Const(input_b), JIRValue::Slice(input_a)) => {
                // We match either variant of const, slice because eq is commutative.
                self.emit_cmp_slice_const(asm_set, out_gpr, input_a, input_b)
            }
            (JIRValue::Slice(input_a), JIRValue::Slice(input_b)) => {
                self.emit_cmp_slice_slice(asm_set, out_gpr, input_a, input_b)
            }
        }
    }

    fn emit_compare(
        &mut self,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
        cmp: fn(u128, u128) -> u128,
        asm_set: fn(&mut MogiAssembler<X64Relocation>, GPRegister),
        asm_set_inverted: fn(&mut MogiAssembler<X64Relocation>, GPRegister),
    ) -> JITResult {
        assert_eq!(output.size(), 1);
        assert_eq!(input_a.size(), input_b.size());
        let out_gpr = self.allocate_slice_gpr(output, SliceType::Output);
        match (input_a, input_b) {
            (JIRValue::Const(input_a), JIRValue::Const(input_b)) => {
                self.emit_copy(output, JIRConst::new(cmp(input_a.value(), input_b.value()), 1))
            }
            (JIRValue::Slice(input_a), JIRValue::Const(input_b)) => {
                self.emit_cmp_slice_const(asm_set, out_gpr, input_a, input_b)
            }
            (JIRValue::Const(input_a), JIRValue::Slice(input_b)) => {
                self.emit_cmp_const_slice(asm_set_inverted, out_gpr, input_a, input_b)
            }
            (JIRValue::Slice(input_a), JIRValue::Slice(input_b)) => {
                self.emit_cmp_slice_slice(asm_set, out_gpr, input_a, input_b)
            }
        }
    }

    fn emit_cmp_const_slice(
        &mut self,
        asm_set_inverted: fn(&mut MogiAssembler<X64Relocation>, GPRegister),
        out_gpr: GPRegister,
        input_a: JIRConst,
        input_b: JIRSlice,
    ) -> JITResult {
        // We swap arguments a and b and perform the inverted comparison such that
        // we can use an immediate if input_a is const. (e.g. a < b is done by comparing b > a)
        let input_b_gpr = self.load_slice_gpr(input_b, SliceType::Input);
        let imm_32 = input_a.value() as i32;
        match input_a.size() {
            1 => assemble!(self.block_mut()?.assembler();
                ; cmp Rb(input_b_gpr as u8), imm_32 as i8
            ),
            2 => assemble!(self.block_mut()?.assembler();
                ; cmp Rw(input_b_gpr as u8), imm_32 as i16
            ),
            4 => assemble!(self.block_mut()?.assembler();
                ; cmp Rd(input_b_gpr as u8), imm_32
            ),
            8 => {
                // For 64bit constants, we need to load the constant into a register.
                let input_a_temp_gpr = self.temp_alloc();
                self.emit_load_const_to_gpr(input_a_temp_gpr, input_a)?;
                assemble!(self.block_mut()?.assembler();
                    ; cmp Rq(input_b_gpr as u8), Rq(input_a_temp_gpr as u8)
                );
                self.temp_free(input_a_temp_gpr)?;
            }
            _ => todo!(),
        }
        asm_set_inverted(self.block_mut()?.assembler(), out_gpr);
        Ok(())
    }

    fn emit_cmp_slice_const(
        &mut self,
        asm_set: fn(&mut MogiAssembler<X64Relocation>, GPRegister),
        out_gpr: GPRegister,
        input_a: JIRSlice,
        input_b: JIRConst,
    ) -> JITResult {
        let input_a_gpr = self.load_slice_gpr(input_a, SliceType::Input);
        let imm_32 = input_b.value() as i32;
        match input_a.size() {
            1 => assemble!(self.block_mut()?.assembler();
                ; cmp Rb(input_a_gpr as u8), imm_32 as i8
            ),
            2 => assemble!(self.block_mut()?.assembler();
                ; cmp Rw(input_a_gpr as u8), imm_32 as i16
            ),
            4 => assemble!(self.block_mut()?.assembler();
                ; cmp Rd(input_a_gpr as u8), imm_32
            ),
            8 => {
                // For 64 bit constants, we need to load the constant into a register.
                let temp_gpr = self.temp_alloc();
                self.emit_load_const_to_gpr(temp_gpr, input_b)?;
                assemble!(self.block_mut()?.assembler();
                    ; cmp Rq(input_a_gpr as u8), Rq(temp_gpr as u8)
                );
                self.temp_free(temp_gpr)?;
            }
            _ => todo!(),
        }
        asm_set(self.block_mut()?.assembler(), out_gpr);
        Ok(())
    }

    fn emit_cmp_slice_slice(
        &mut self,
        asm_set: fn(&mut MogiAssembler<X64Relocation>, GPRegister),
        out_gpr: GPRegister,
        input_a: JIRSlice,
        input_b: JIRSlice,
    ) -> JITResult {
        let input_a_gpr = self.load_slice_gpr(input_a, SliceType::Input);
        let input_b_gpr = self.load_slice_gpr(input_b, SliceType::Input);
        match input_a.size() {
            1 => assemble!(self.block_mut()?.assembler();
                ; cmp Rb(input_a_gpr as u8), Rb(input_b_gpr as u8)
            ),
            2 => assemble!(self.block_mut()?.assembler();
                ; cmp Rw(input_a_gpr as u8), Rw(input_b_gpr as u8)
            ),
            4 => assemble!(self.block_mut()?.assembler();
                ; cmp Rd(input_a_gpr as u8), Rd(input_b_gpr as u8)
            ),
            8 => assemble!(self.block_mut()?.assembler();
                ; cmp Rq(input_a_gpr as u8), Rq(input_b_gpr as u8)
            ),
            _ => todo!(),
        }
        asm_set(self.block_mut()?.assembler(), out_gpr);
        Ok(())
    }
}
