use crate::jit::api::{
    JITResult,
    JitError,
};
use crate::jit::x86::regalloc::{
    RegisterAllocationType,
    SpillBehavior,
};
use crate::jit::x86::{
    GPRegister,
    RegisterLoadInfo,
    X86JITRuntimeContext,
    X86MogiJit,
    assemble,
};
use mogi_sleigh::spec::jir::common::JIRSize;
use mogi_sleigh::spec::jir::values::{
    JIRConst,
    JIRSlice,
    JIRValue,
};
use std::cmp::min;
use tracing::trace;

impl X86MogiJit {
    /// Emit code to load the requested slice into a [GPRegister] if necessary.
    /// If the slice is already loaded, the register is returned.
    /// Otherwise, code is emitted that loads the slice into a free register,
    /// which is then returned.
    #[inline]
    pub(super) fn load_slice_gpr(
        &mut self,
        slice: JIRSlice,
    ) -> GPRegister {
        self.load_slice_gpr_with_hint(None, slice)
    }

    /// Works like [load_slice_gpr], but tries to load the [JIRSlice] to
    /// the provided hint [GPRegister] if it is available.
    #[inline]
    pub(super) fn load_slice_gpr_with_hint(
        &mut self,
        hint: Option<GPRegister>,
        slice: JIRSlice,
    ) -> GPRegister {
        let register = self
            .allocate_gp(hint, RegisterAllocationType::Slice(slice), SpillBehavior::Spill)
            .expect("Since we directed the allocation to spill if no space is available, this has to succeed");
        match register {
            RegisterLoadInfo::Unloaded(register) => {
                self.load_slice_from_runtime_context(register, slice);
                register
            }
            RegisterLoadInfo::Loaded(register) => {
                trace!("Slice {slice:?} is already loaded in GP register {register:?}");
                register
            }
            RegisterLoadInfo::NeedRelocation { .. } => {
                panic!("This case cannot happen when allocating a slice")
            }
        }
    }

    /// Emit instructions to move the provided slice from the runtime context to the provided gpr.
    #[inline]
    fn load_slice_from_runtime_context(
        &mut self,
        register: GPRegister,
        slice: JIRSlice,
    ) {
        // The register has not been loaded before.
        // We need to insert an instruction that loads it from context.
        // For that we first need to calculate the offset in the cpu context.
        let varnode_offset = X86JITRuntimeContext::varnode_offset(slice);
        // Emit the instruction that loads the varnode from the specified space in the context.
        match slice.size() {
            1 => {
                assemble!(self.block_mut().unwrap().assembler;
                    ; movzx Rq(register as u8), BYTE [cpu_context + varnode_offset as _]
                );
            }
            2 => {
                assemble!(self.block_mut().unwrap().assembler;
                    ; movzx Rq(register as u8), WORD [cpu_context + varnode_offset as _]
                );
            }
            4 => {
                // Moving to the lower 32 bits of a register zeros the upper 32 bits
                assemble!(self.block_mut().unwrap().assembler;
                    ; mov Rd(register as u8), DWORD [cpu_context + varnode_offset as _]
                );
            }
            8 => {
                assemble!(self.block_mut().unwrap().assembler;
                    ; mov Rq(register as u8), QWORD [cpu_context + varnode_offset as _]
                );
            }
            _ => {
                panic!("Unsupported load size")
            }
        }
    }

    /// Emit instructions to copy the source [JIRValue] into the target [JIRSlice].
    #[inline]
    pub fn emit_copy(
        &mut self,
        target: JIRSlice,
        source: impl Into<JIRValue>,
    ) -> JITResult {
        let source = source.into();
        debug_assert_eq!(target.size(), source.size());

        let size = target.size();

        if target == source {
            // If target==source we just need to make sure the target is loaded.
            self.load_slice_gpr(target);
            return Ok(());
        }

        // Allocate and associate a GP register with the slice.
        // No load is needed, since we overwrite the data anyway for a copy.
        let target_reg = self.get_slice_gp(target);
        self.emit_load_value_to_gpr(target_reg, source)
    }

    /// Load the provided value into the target [GPRegister].
    /// The [GPRegister] needs to have been allocated or loaded first,
    /// otherwise the behavior is undefined.
    #[inline]
    pub(super) fn emit_load_value_to_gpr(
        &mut self,
        target_gpr: GPRegister,
        source: JIRValue,
    ) -> JITResult {
        match source {
            JIRValue::Slice(slice) => {
                self.emit_load_slice_to_gpr(target_gpr, slice)?;
            }
            JIRValue::Const(value) => {
                self.emit_load_const_to_gpr(target_gpr, value)?;
            }
        }
        Ok(())
    }

    /// Load the [JIRSlice] `slice` into the [GPRegister] `target_reg`.
    /// If the slice is already loaded in `target_reg` this emits nothing.
    #[inline]
    pub(super) fn emit_load_slice_to_gpr(
        &mut self,
        target_reg: GPRegister,
        slice: JIRSlice,
    ) -> JITResult {
        let size = slice.size();
        if size <= 8 {
            // TODO: It might make sense to be able to hint the gpr to load to for load_slice_gp.
            //       We could omit a few mov instructions in some cases that way.
            //       I.e. if the slice is not loaded yet, use the provided hint, and otherwise
            //       we just return the gpr to which the slice was already loaded.
            let reg = self.load_slice_gpr_with_hint(Some(target_reg), slice);
            self.copy_gpr_to_gpr(target_reg, reg, size)?;
        } else {
            todo!("Support sizes > 8")
        }
        Ok(())
    }

    #[inline]
    pub fn copy_gpr_to_gpr(
        &mut self,
        target: GPRegister,
        source: GPRegister,
        size: JIRSize,
    ) -> JITResult {
        // Only emit a copy instruction, if target and source are different.
        if target == source {
            return Ok(());
        }
        match size {
            1..=4 => {
                assemble!(self.block_mut()?.assembler;
                    ; mov Rd(target as u8), Rd(source as u8)
                );
            }
            5..=8 => {
                assemble!(self.block_mut()?.assembler;
                    ; mov Rq(target as u8), Rq(source as u8)
                );
            }
            _ => unimplemented!("Unsupported size for gpr copy"),
        }
        Ok(())
    }

    /// Emit an instruction that loads the provided constant into the provided target gp register.
    #[inline]
    pub(super) fn emit_load_const_to_gpr(
        &mut self,
        target_reg: GPRegister,
        value: JIRConst,
    ) -> JITResult {
        let imm_size = min((u128::BITS - value.value().leading_zeros() + 7) / 8, value.size().into());
        let size = value.size();
        let value = value.value();
        if value == 0 {
            // Zeroing out a register can be done more efficiently
            return self.emit_zero_gpr(target_reg, size);
        }
        let block = self.block_mut()?;
        match size {
            1 => {
                // Move to 32 bit register to zero extend.
                assemble!(block.assembler;
                    ; mov Rd(target_reg as u8), BYTE value as _
                );
            }
            2 => {
                // Move to 32 bit register to zero extend.
                assemble!(block.assembler;
                    ; mov Rd(target_reg as u8), WORD value as _
                );
            }
            3..=4 => {
                // We need to move to 32 bit register here, since the 64 bit move sign extends?????
                assemble!(block.assembler;
                    ; mov Rd(target_reg as u8), DWORD value as _
                );
            }
            5..=8 => {
                assemble!(block.assembler;
                    ; mov Rq(target_reg as u8), QWORD value as _
                );
            }
            size => {
                todo!("Unsupported size {size}")
            }
        }
        Ok(())
    }

    /// Emit instructions to zero out the target register.
    /// Uses the size to determine the best instruction encoding.
    #[inline]
    fn emit_zero_gpr(
        &mut self,
        target: GPRegister,
        size: JIRSize,
    ) -> JITResult {
        match size {
            1..=8 => {
                // For now we just zero the entire register.
                // in the future it might make sense to do more specific stuff here
                // For now, it is enough to do xor r32, r32.
                // This also zeros the upper 32 bits for 64 bit registers (see intel sdm)
                assemble!(self.block_mut()?.assembler;
                    ; xor Rd(target as u8), Rd(target as u8)
                );
            }
            _ => {
                panic!("Unsupported size")
            }
        }
        Ok(())
    }

    /// Emits instructions to load the input values into registers.
    /// If the output appears on the rhs, then the output register is simply reused.
    /// If the output does not appear on the rhs, then one of the inputs is copied into
    /// the output, since most x86 instructions require that one of the operands is also the output.
    #[inline]
    pub(super) fn load_commutative_inputs(
        &mut self,
        output: JIRSlice,
        input_a: JIRSlice,
        input_b: JIRSlice,
    ) -> Result<(GPRegister, GPRegister), JitError> {
        let (target_gp, operand_gp) = if output == input_a {
            (self.load_slice_gpr(input_a), self.load_slice_gpr(input_b))
        } else if output == input_b {
            (self.load_slice_gpr(input_b), self.load_slice_gpr(input_a))
        } else {
            // Copy one of the operands to the output and then do the and operation
            self.emit_copy(output, input_a)?;
            (self.load_slice_gpr(output), self.load_slice_gpr(input_b))
        };
        Ok((target_gp, operand_gp))
    }
}
