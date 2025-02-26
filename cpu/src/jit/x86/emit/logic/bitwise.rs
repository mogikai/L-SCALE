use crate::jit::api::JITResult;
use crate::jit::x86::regalloc::SliceType;
use crate::jit::x86::registers::GPRegister;
use crate::jit::x86::{
    X86MogiJit,
    assemble,
};
use mogi_sleigh::spec::jir::common::JIRSize;
use mogi_sleigh::spec::jir::values::{
    JIRConst,
    JIRSlice,
    JIRValue,
};

impl<PageDataT> X86MogiJit<PageDataT> {
    pub fn emit_bitwise_and(
        &mut self,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        self.emit_bitwise_op(
            output,
            input_a,
            input_b,
            |a, b| JIRConst::new(a.value() & b.value(), a.size()),
            |jit, output_gp, immediate, size| {
                // Choose the smallest encoding that still fits
                match size {
                    0 => {
                        panic!("Zero sized slices are not allowed")
                    }
                    1 => {
                        assemble!(jit.block_mut()?.assembler();
                            ; and Rq(output_gp as u8), BYTE immediate as _
                        );
                    }
                    2 => {
                        assemble!(jit.block_mut()?.assembler();
                            ; and Rq(output_gp as u8), WORD immediate as _
                        );
                    }
                    3..=4 => {
                        assemble!(jit.block_mut()?.assembler();
                            ; and Rq(output_gp as u8), DWORD immediate as _
                        );
                    }
                    _ => {
                        unreachable!("This code is not reachable because we checked beforehand, that the size is <= 4")
                    }
                }
                Ok(())
            },
            |jit, output_gp, operand_gp, _size| {
                assemble!(jit.block_mut()?.assembler();
                    ; and Rq(output_gp as u8), Rq(operand_gp as u8)
                );
                Ok(())
            },
        )
    }

    pub fn emit_bitwise_or(
        &mut self,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        self.emit_bitwise_op(
            output,
            input_a,
            input_b,
            |a, b| JIRConst::new(a.value() | b.value(), a.size()),
            |jit, output_gp, immediate, size| {
                // Choose the smallest encoding that still fits
                match size {
                    0 => {
                        panic!("Zero sized slices are not allowed")
                    }
                    1 => {
                        assemble!(jit.block_mut()?.assembler();
                            ; or Rq(output_gp as u8), BYTE immediate as _
                        );
                    }
                    2 => {
                        assemble!(jit.block_mut()?.assembler();
                            ; or Rq(output_gp as u8), WORD immediate as _
                        );
                    }
                    3..=4 => {
                        assemble!(jit.block_mut()?.assembler();
                            ; or Rq(output_gp as u8), DWORD immediate as _
                        );
                    }
                    _ => {
                        unreachable!("This code is not reachable because we checked beforehand, that the size is <= 4")
                    }
                }
                Ok(())
            },
            |jit, output_gp, operand_gp, _size| {
                assemble!(jit.block_mut()?.assembler();
                    ; or Rq(output_gp as u8), Rq(operand_gp as u8)
                );
                Ok(())
            },
        )
    }

    pub fn emit_bitwise_xor(
        &mut self,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        self.emit_bitwise_op(
            output,
            input_a,
            input_b,
            |a, b| JIRConst::new(a.value() ^ b.value(), a.size()),
            |jit, output_gp, immediate, size| {
                // Choose the smallest encoding that still fits
                match size {
                    0 => {
                        panic!("Zero sized slices are not allowed")
                    }
                    1 => {
                        assemble!(jit.block_mut()?.assembler();
                            ; xor Rq(output_gp as u8), BYTE immediate as _
                        );
                    }
                    2 => {
                        assemble!(jit.block_mut()?.assembler();
                            ; xor Rq(output_gp as u8), WORD immediate as _
                        );
                    }
                    3..=4 => {
                        assemble!(jit.block_mut()?.assembler();
                            ; xor Rq(output_gp as u8), DWORD immediate as _
                        );
                    }
                    _ => {
                        unreachable!("This code is not reachable because we checked beforehand, that the size is <= 4")
                    }
                }
                Ok(())
            },
            |jit, output_gp, operand_gp, _size| {
                assemble!(jit.block_mut()?.assembler();
                    ; xor Rq(output_gp as u8), Rq(operand_gp as u8)
                );
                Ok(())
            },
        )
    }

    pub fn emit_bitwise_op(
        &mut self,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
        op_const: fn(JIRConst, JIRConst) -> JIRConst,
        op_gp_imm: fn(&mut X86MogiJit<PageDataT>, GPRegister, u128, JIRSize) -> JITResult,
        op_gp_gp: fn(&mut X86MogiJit<PageDataT>, GPRegister, GPRegister, JIRSize) -> JITResult,
    ) -> JITResult {
        assert_eq!(input_a.size(), input_b.size());
        // TODO: It might make sense to directly put the memory reference into the instruction, instead of loading
        //       to a register first. However, this requires that we know if the value is used in subsequent instructions
        //       because if it is used, we have to load it in a register. The exception to this is for cases like
        //       out = out OP b, where out does not have to be loaded first and we can just use the mem reference directly.
        match (input_a, input_b) {
            (JIRValue::Const(input_a), JIRValue::Const(input_b)) => {
                // In the const case we can simply calculate the value ahead of time and move it into the output.
                self.emit_copy(output, op_const(input_a, input_b))?
            }
            (JIRValue::Slice(input_a), JIRValue::Const(input_b))
            | (JIRValue::Const(input_b), JIRValue::Slice(input_a)) => {
                // The order of arguments does not matter, so we can simply handle both cases here
                let immediate = input_b.value();
                let immediate_size = input_b.size();
                if immediate_size <= 4 {
                    // Copy the input into the output, since x86 uses the first operand as output.
                    self.emit_copy(output, input_a)?;
                    let output_gp = self.load_slice_gpr(output, SliceType::Output);
                    op_gp_imm(self, output_gp, immediate, immediate_size)?;
                } else if immediate_size <= 8 {
                    // Immediates up to 64 bits need to be moved into a register first.
                    let (output_gp, operand_gp) = if output != input_a {
                        // If the output is not input_a, we can just copy the immediate into the output
                        self.emit_copy(output, input_b)?;
                        let input_a_gp = self.load_slice_gpr(input_a, SliceType::Input);
                        // CAVEAT: the load needs to happen here, otherwise we perform unnecessary operations.
                        let output_gp = self.load_slice_gpr(output, SliceType::Output);
                        (output_gp, input_a_gp)
                    } else {
                        // IF output==input_a we have to move the immediate to a temporary
                        let tmp = self.get_temp_gpr();
                        self.emit_load_const_to_gpr(tmp, input_b)?;
                        let output_gp = self.load_slice_gpr(output, SliceType::Output);
                        (output_gp, tmp)
                    };
                    op_gp_gp(self, output_gp, operand_gp, immediate_size)?;
                } else {
                    todo!("Unsupported operation size")
                }
            }
            (JIRValue::Slice(input_a), JIRValue::Slice(input_b)) => {
                let (target_gp, operand_gp) = self.load_commutative_inputs(output, input_a, input_b)?;
                op_gp_gp(self, target_gp, operand_gp, input_a.size())?;
            }
        }
        Ok(())
    }

    pub fn emit_bitwise_negate(
        &mut self,
        output: JIRSlice,
        input_a: JIRValue,
    ) -> JITResult {
        assert_eq!(output.size(), input_a.size());
        // TODO: It might make sense to directly put the memory reference into the instruction, instead of loading
        //       to a register first. However, this requires that we know if the value is used in subsequent instructions
        //       because if it is used, we have to load it in a register. The exception to this is for cases like
        //       out = out OP b, where out does not have to be loaded first and we can just use the mem reference directly.
        match input_a {
            JIRValue::Const(input_a) => {
                // In the const case we can simply calculate the value ahead of time and move it into the output.
                self.emit_copy(output, JIRConst::new(!input_a.value(), input_a.size()))?;
            }
            JIRValue::Slice(input_a) => {
                self.emit_copy(output, input_a)?;
                let output_gpr = self.load_slice_gpr(output, SliceType::Output);
                assemble!(self.block_mut()?.assembler();
                    ; not Rq(output_gpr as u8)
                );
            }
        }
        Ok(())
    }
}
