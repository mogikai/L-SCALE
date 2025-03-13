use crate::jit::api::JITResult;
use crate::jit::x86::assemble;
use crate::jit::x86::registers::general_purpose::GPRegister;
use crate::jit::x86::x64emitter::X64Emitter;
use crate::jit::x86::x64emitter::regalloc::SliceType;
use mogi_sleigh::spec::jir::common::JIRSize;
use mogi_sleigh::spec::jir::values::{
    JIRConst,
    JIRSlice,
    JIRValue,
};

enum FlagType {
    Overflow,
    Carry,
    Zero,
}

impl X64Emitter<'_> {
    pub fn emit_scarry(
        &mut self,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        assert_eq!(output.size(), 1);
        assert_eq!(input_a.size(), input_b.size());
        match (input_a, input_b) {
            (JIRValue::Const(input_a), JIRValue::Const(input_b)) => self.emit_add_flag_const_const_signed(
                output,
                input_a.value() as i128,
                input_b.value() as i128,
                input_a.size(),
            ),
            (JIRValue::Slice(input_a), JIRValue::Const(input_b))
            | (JIRValue::Const(input_b), JIRValue::Slice(input_a)) => {
                self.emit_add_flag_slice_const(output, input_a, input_b, FlagType::Overflow)
            }
            (JIRValue::Slice(input_a), JIRValue::Slice(input_b)) => {
                self.emit_add_flag_slice_slice(output, input_a, input_b, FlagType::Overflow)
            }
        }
    }

    pub fn emit_sborrow(
        &mut self,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        assert_eq!(output.size(), 1);
        assert_eq!(input_a.size(), input_b.size());
        match (input_a, input_b) {
            (JIRValue::Const(input_a), JIRValue::Const(input_b)) => self.emit_sub_flag_const_const_signed(
                output,
                input_a.value() as i128,
                input_b.value() as i128,
                input_a.size(),
            ),
            (JIRValue::Slice(input_a), JIRValue::Const(input_b)) => {
                self.emit_sub_flag_slice_const(output, input_a, input_b, FlagType::Overflow)
            }
            (input_a, JIRValue::Slice(input_b)) => {
                // TODO: The const, slice case can be further optimized to use immediates instead of loading
                //       into a gpr.
                self.emit_sub_flag_value_slice(output, input_a, input_b, FlagType::Overflow)
            }
        }
    }

    pub fn emit_carry(
        &mut self,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        assert_eq!(output.size(), 1);
        assert_eq!(input_a.size(), input_b.size());
        match (input_a, input_b) {
            (JIRValue::Const(input_a), JIRValue::Const(input_b)) => {
                self.emit_add_flag_const_const_unsigned(output, input_a.value(), input_b.value(), input_a.size())
            }
            (JIRValue::Slice(input_a), JIRValue::Const(input_b))
            | (JIRValue::Const(input_b), JIRValue::Slice(input_a)) => {
                // addition is commutative, hence we can just swap the const and slice arguments.
                self.emit_add_flag_slice_const(output, input_a, input_b, FlagType::Carry)
            }
            (JIRValue::Slice(input_a), JIRValue::Slice(input_b)) => {
                self.emit_add_flag_slice_slice(output, input_a, input_b, FlagType::Carry)
            }
        }
    }

    fn emit_add_flag_const_const_unsigned(
        &mut self,
        output: JIRSlice,
        input_a: u128,
        input_b: u128,
        size: JIRSize,
    ) -> JITResult {
        let (val, mut overflow) = input_a.overflowing_add(input_b);
        // Overflow is only set if 128 bit addition overflows.
        // We now check whether the result is larger than the allowed value.
        // If that is the case, an overflow occurred for the smaller const.
        let max = (1 << (size * 8)) - 1;
        if val > max {
            overflow = true;
        }
        self.emit_copy(output, JIRConst::new(overflow as u128, 1))
    }
    fn emit_add_flag_const_const_signed(
        &mut self,
        output: JIRSlice,
        input_a: i128,
        input_b: i128,
        size: JIRSize,
    ) -> JITResult {
        let (val, mut overflow) = input_a.overflowing_add(input_b);
        // Overflow is only set if 128 bit addition overflows.
        // We now check whether the result is larger than the allowed value.
        // If that is the case, an overflow occurred for the smaller const.
        let max = (1 << (size * 8 - 1)) - 1;
        let min = !max;
        if input_a > 0 && input_b > 0 && val > max {
            overflow = true;
        }
        if input_a < 0 && input_b < 0 && val < min {
            overflow = true;
        }
        self.emit_copy(output, JIRConst::new(overflow as u128, 1))
    }

    fn emit_add_flag_slice_const(
        &mut self,
        output: JIRSlice,
        input_a: JIRSlice,
        input_b: JIRConst,
        flag_type: FlagType,
    ) -> JITResult {
        // We store the output into a temporary because we don't need it.
        let temp_gpr = self.get_temp_gpr();
        let input_a_gpr = self.emit_load_slice_to_gpr(temp_gpr, input_a, SliceType::Input);
        match input_a.size() {
            1 => assemble!(self.block.assembler();
                ; add Rb(temp_gpr as u8), input_b.value() as i8
            ),
            2 => assemble!(self.block.assembler();
                ; add Rw(temp_gpr as u8), input_b.value() as i16
            ),
            4 => assemble!(self.block.assembler();
                ; add Rd(temp_gpr as u8), input_b.value() as i32
            ),
            8 => {
                // There are no 64 bit immediates for add, hence we need to load the immediate into a temp first.
                let input_b_gpr = self.temp_alloc();
                self.emit_load_const_to_gpr(input_b_gpr, input_b)?;
                assemble!(self.block.assembler();
                    ; add Rq(temp_gpr as u8), Rq(input_b_gpr as u8)
                );
                self.temp_free(input_b_gpr)?;
            }
            _ => todo!("Unsupported size"),
        }
        let out_gpr = self.allocate_slice_gpr(output, SliceType::Output);
        self.emit_set_flag(out_gpr, flag_type)
    }

    fn emit_add_flag_slice_slice(
        &mut self,
        output: JIRSlice,
        input_a: JIRSlice,
        input_b: JIRSlice,
        flag_type: FlagType,
    ) -> JITResult {
        assert_eq!(output.size(), 1);
        assert_eq!(input_a.size(), input_b.size());
        // We store the output into a temporary because we don't need it.
        let temp_gpr = self.get_temp_gpr();
        let input_a_gpr = self.emit_load_slice_to_gpr(temp_gpr, input_a, SliceType::Input);
        let input_b_gpr = self.load_slice_gpr(input_b, SliceType::Input);
        match input_a.size() {
            1 => assemble!(self.block.assembler();
                ; add Rb(temp_gpr as u8), Rb(input_b_gpr as u8)
            ),
            2 => assemble!(self.block.assembler();
                ; add Rw(temp_gpr as u8), Rw(input_b_gpr as u8)
            ),
            4 => assemble!(self.block.assembler();
                ; add Rd(temp_gpr as u8), Rd(input_b_gpr as u8)
            ),
            8 => assemble!(self.block.assembler();
                ; add Rq(temp_gpr as u8), Rq(input_b_gpr as u8)
            ),
            _ => todo!("Unsupported size"),
        }
        let out_gpr = self.allocate_slice_gpr(output, SliceType::Output);
        self.emit_set_flag(out_gpr, flag_type)
    }

    fn emit_sub_flag_value_slice(
        &mut self,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRSlice,
        flag_type: FlagType,
    ) -> JITResult {
        let input_a_gpr = match input_a {
            JIRValue::Const(input_a) => {
                let temp_gpr = self.get_temp_gpr();
                self.emit_load_const_to_gpr(temp_gpr, input_a)?;
                temp_gpr
            }
            JIRValue::Slice(input_a) => self.load_slice_gpr(input_a, SliceType::Input),
        };
        let input_b_gpr = self.load_slice_gpr(input_b, SliceType::Input);
        match input_a.size() {
            1 => assemble!(self.block.assembler();
                ; cmp Rb(input_a_gpr as u8), Rb(input_b_gpr as u8)
            ),
            2 => assemble!(self.block.assembler();
                ; cmp Rw(input_a_gpr as u8), Rw(input_b_gpr as u8)
            ),
            4 => assemble!(self.block.assembler();
                ; cmp Rd(input_a_gpr as u8), Rd(input_b_gpr as u8)
            ),
            8 => assemble!(self.block.assembler();
                ; cmp Rq(input_a_gpr as u8), Rq(input_b_gpr as u8)
            ),
            _ => todo!("Unsupported size"),
        }
        let out_gpr = self.allocate_slice_gpr(output, SliceType::Output);
        self.emit_set_flag(out_gpr, flag_type)
    }

    fn emit_sub_flag_slice_const(
        &mut self,
        output: JIRSlice,
        input_a: JIRSlice,
        input_b: JIRConst,
        flag_type: FlagType,
    ) -> JITResult {
        // We store the output into a temporary because we don't need it.
        let temp_gpr = self.get_temp_gpr();
        let input_a_gpr = self.emit_load_slice_to_gpr(temp_gpr, input_a, SliceType::Input);
        match input_a.size() {
            1 => assemble!(self.block.assembler();
                ; cmp Rb(temp_gpr as u8), input_b.value() as i8
            ),
            2 => assemble!(self.block.assembler();
                ; cmp Rw(temp_gpr as u8), input_b.value() as i16
            ),
            4 => assemble!(self.block.assembler();
                ; cmp Rd(temp_gpr as u8), input_b.value() as i32
            ),
            8 => {
                // There are no 64 bit immediates for add, hence we need to load the immediate into a temp first.
                let input_b_gpr = self.temp_alloc();
                self.emit_load_const_to_gpr(input_b_gpr, input_b)?;
                assemble!(self.block.assembler();
                    ; cmp Rq(temp_gpr as u8), Rq(input_b_gpr as u8)
                );
                self.temp_free(input_b_gpr)?;
            }
            _ => todo!("Unsupported size"),
        }
        let out_gpr = self.allocate_slice_gpr(output, SliceType::Output);
        self.emit_set_flag(out_gpr, flag_type)
    }

    fn emit_sub_flag_const_const_signed(
        &mut self,
        output: JIRSlice,
        input_a: i128,
        input_b: i128,
        size: JIRSize,
    ) -> JITResult {
        let (val, mut overflow) = input_a.overflowing_sub(input_b);
        // Overflow is only set if 128 bit addition overflows.
        // We now check whether the result is larger than the allowed value.
        // If that is the case, an overflow occurred for the smaller const.
        let max = (1 << (size * 8 - 1)) - 1;
        let min = !max;
        if input_a >= 0 && input_b < 0 && val > max {
            overflow = true;
        }
        if input_a <= 0 && input_b > 0 && val < min {
            overflow = true;
        }
        self.emit_copy(output, JIRConst::new(overflow as u128, 1))
    }

    fn emit_set_flag(
        &mut self,
        flag_gpr: GPRegister,
        flag_type: FlagType,
    ) -> JITResult {
        match flag_type {
            FlagType::Overflow => assemble!(self.block.assembler();
                ; seto Rb(flag_gpr as u8)
            ),
            FlagType::Carry => assemble!(self.block.assembler();
                ; setc Rb(flag_gpr as u8)
            ),
            FlagType::Zero => assemble!(self.block.assembler();
                ; setz Rb(flag_gpr as u8)
            ),
        }
        Ok(())
    }
}
