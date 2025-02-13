use crate::jit::api::JITResult;
use crate::jit::x86::regalloc::SliceType;
use crate::jit::x86::registers::GPRegister;
use crate::jit::x86::{
    X86MogiJit,
    assemble,
};
use mogi_sleigh::spec::jir::values::{
    JIRSlice,
    JIRValue,
};

enum FlagType {
    Overflow,
    Carry,
    Zero,
}

impl X86MogiJit {
    pub fn emit_scarry(
        &mut self,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        match (input_a, input_b) {
            (JIRValue::Const(input_a), JIRValue::Const(input_b)) => todo!(),
            (JIRValue::Slice(input_a), JIRValue::Const(input_b)) => todo!(),
            (JIRValue::Const(input_a), JIRValue::Slice(input_b)) => todo!(),
            (JIRValue::Slice(input_a), JIRValue::Slice(input_b)) => {
                self.emit_flag_slice_slice(output, input_a, input_b, FlagType::Overflow)
            }
        }
    }

    pub fn emit_carry(
        &mut self,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        match (input_a, input_b) {
            (JIRValue::Const(input_a), JIRValue::Const(input_b)) => todo!(),
            (JIRValue::Slice(input_a), JIRValue::Const(input_b)) => todo!(),
            (JIRValue::Const(input_a), JIRValue::Slice(input_b)) => todo!(),
            (JIRValue::Slice(input_a), JIRValue::Slice(input_b)) => {
                self.emit_flag_slice_slice(output, input_a, input_b, FlagType::Carry)
            }
        }
    }

    fn emit_flag_slice_slice(
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
            1 => assemble!(self.block_mut()?.assembler();
                ; add Rb(temp_gpr as u8), Rb(input_b_gpr as u8)
            ),
            2 => assemble!(self.block_mut()?.assembler();
                ; add Rw(temp_gpr as u8), Rw(input_b_gpr as u8)
            ),
            4 => assemble!(self.block_mut()?.assembler();
                ; add Rd(temp_gpr as u8), Rd(input_b_gpr as u8)
            ),
            8 => assemble!(self.block_mut()?.assembler();
                ; add Rq(temp_gpr as u8), Rq(input_b_gpr as u8)
            ),
            _ => todo!("Unsupported size"),
        }
        let out_gpr = self.allocate_slice_gpr(output, SliceType::Output);
        self.emit_set_flag(out_gpr, flag_type)
    }

    fn emit_set_flag(
        &mut self,
        flag_gpr: GPRegister,
        flag_type: FlagType,
    ) -> JITResult {
        match flag_type {
            FlagType::Overflow => assemble!(self.block_mut()?.assembler();
                ; seto Rb(flag_gpr as u8)
            ),
            FlagType::Carry => assemble!(self.block_mut()?.assembler();
                ; setc Rb(flag_gpr as u8)
            ),
            FlagType::Zero => assemble!(self.block_mut()?.assembler();
                ; setz Rb(flag_gpr as u8)
            ),
        }
        Ok(())
    }
}
