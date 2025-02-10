use crate::jit::api::{
    JITResult,
    JitError,
};
use crate::jit::trace_jit;
use crate::jit::x86::regalloc::{
    RegisterAllocationHint,
    RegisterAllocationType,
    SliceType,
    SpillBehavior,
};
use crate::jit::x86::registers::{
    GPRegister,
    RegisterAllocation,
    RegisterLoadInfo,
};
use crate::jit::x86::task_context::X86TaskContext;
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
use std::cmp::min;
use tracing::trace;

#[derive(Debug)]
pub enum CopyExtendMode {
    /// Extend to the provided size
    ZeroExtend(JIRSize),
    /// Sign extend to the provided size.
    SignExtend(JIRSize),
    None,
}

impl X86MogiJit {
    /// Emit code to load the requested slice into a [GPRegister] if necessary.
    /// If the slice is already loaded, the register is returned.
    /// Otherwise, code is emitted that loads the slice into a free register,
    /// which is then returned.
    #[inline]
    pub(super) fn load_slice_gpr(
        &mut self,
        slice: JIRSlice,
        slice_type: SliceType,
    ) -> GPRegister {
        self.load_slice_gpr_with_hint(RegisterAllocationHint::None, slice, slice_type)
    }

    /// Works like [load_slice_gpr], but tries to load the [JIRSlice] to
    /// the provided hint [GPRegister] if it is available.
    #[inline]
    pub(super) fn load_slice_gpr_with_hint(
        &mut self,
        hint: RegisterAllocationHint,
        slice: JIRSlice,
        slice_type: SliceType,
    ) -> GPRegister {
        let alloc_type = match slice_type {
            SliceType::Input => RegisterAllocationType::Slice(slice),
            SliceType::Output => RegisterAllocationType::OutSlice(slice),
        };
        let register = self
            .allocate_gpr(hint, alloc_type, SpillBehavior::Spill)
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

    /// Directly load the provided [JIRSlice] into the provided target [GPRegister].
    /// This moves any existing slice ot of the target [GPRegister], potentially spilling it.
    /// Any existing references to registers may be invalidated by this operation, so you
    /// have to fetch these references again to avoid accessing invalid registers.
    #[inline]
    pub(super) fn load_slice_gpr_exact(
        &mut self,
        gpr: GPRegister,
        slice: JIRSlice,
        slice_type: SliceType,
    ) -> JITResult<GPRegister> {
        // Make sure the target gpr is free for our allocation.
        self.move_slice_from_gp(gpr)?;
        let alloc_type = match slice_type {
            SliceType::Input => RegisterAllocationType::Slice(slice),
            SliceType::Output => RegisterAllocationType::OutSlice(slice),
        };
        let register = self
            .allocate_gpr(RegisterAllocationHint::Reallocate(gpr), alloc_type, SpillBehavior::Fail)
            .expect("This cannot fail, since we just made sure that the gpr is free by moving out of it.");
        Ok(match register {
            RegisterLoadInfo::Unloaded(register) => {
                self.load_slice_from_runtime_context(register, slice);
                register
            }
            RegisterLoadInfo::Loaded(register) => {
                trace_jit!("Slice {slice:?} is already loaded in GP register {register:?}");
                register
            }
            RegisterLoadInfo::NeedRelocation { from, to } => {
                trace_jit!("Slice {slice:?} needs to be relocated: {to:?}<-{from:?}");
                self.copy_gpr_to_gpr(to, from, slice.size(), CopyExtendMode::None)?;
                to
            }
        })
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
        let varnode_offset = X86TaskContext::varnode_offset(slice);
        trace!("Loading slice {slice:?} that refers to varnode {:?} into {register:?}", slice.varnode_id);
        // Emit the instruction that loads the varnode from the specified space in the context.
        match slice.size() {
            1 => {
                assemble!(self.block_mut().unwrap().assembler();
                    ; movzx Rq(register as u8), BYTE [cpu_context + varnode_offset as _]
                );
            }
            2 => {
                assemble!(self.block_mut().unwrap().assembler();
                    ; movzx Rq(register as u8), WORD [cpu_context + varnode_offset as _]
                );
            }
            4 => {
                // Moving to the lower 32 bits of a register zeros the upper 32 bits
                assemble!(self.block_mut().unwrap().assembler();
                    ; mov Rd(register as u8), DWORD [cpu_context + varnode_offset as _]
                );
            }
            8 => {
                assemble!(self.block_mut().unwrap().assembler();
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
            self.load_slice_gpr(target, SliceType::Output);
            return Ok(());
        }

        // Allocate and associate a GP register with the slice.
        // No load is needed, since we overwrite the data anyway for a copy.
        let target_reg = self.allocate_slice_gpr(target, SliceType::Output);
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
        // TODO: Fix loading to existing gpr. When eax is specified, the value is first loaded to a different register.
        match source {
            JIRValue::Slice(slice) => {
                // Values can only appear on the right hand side of an expression and hence must be inputs.
                self.emit_load_slice_to_gpr(target_gpr, slice, SliceType::Input)?;
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
        slice_type: SliceType,
    ) -> JITResult {
        let size = slice.size();
        if size <= 8 {
            // TODO: It might make sense to be able to hint the gpr to load to for load_slice_gp.
            //       We could omit a few mov instructions in some cases that way.
            //       I.e. if the slice is not loaded yet, use the provided hint, and otherwise
            //       we just return the gpr to which the slice was already loaded.
            let reg = self.load_slice_gpr_with_hint(RegisterAllocationHint::Allocate(target_reg), slice, slice_type);
            self.copy_gpr_to_gpr(target_reg, reg, size, CopyExtendMode::None)?;
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
        extend_mode: CopyExtendMode,
    ) -> JITResult {
        match extend_mode {
            CopyExtendMode::ZeroExtend(new_size) => {
                if target == source && size == new_size {
                    trace_jit!("Zero extend with equal sizes: no-op");
                    return Ok(());
                }
                trace_jit!("Zero extending from {source:?}:{size} to {target:?}:{new_size}");
                Ok(())
            }
            CopyExtendMode::SignExtend(new_size) => {
                todo!()
            }
            CopyExtendMode::None => {
                // Only emit a copy instruction, if target and source are different.
                if target == source {
                    return Ok(());
                }
                match size {
                    1..=4 => {
                        assemble!(self.block_mut()?.assembler();
                            ; mov Rd(target as u8), Rd(source as u8)
                        );
                    }
                    5..=8 => {
                        assemble!(self.block_mut()?.assembler();
                            ; mov Rq(target as u8), Rq(source as u8)
                        );
                    }
                    _ => unimplemented!("Unsupported size for gpr copy"),
                }
                Ok(())
            }
        }
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
                assemble!(block.assembler();
                    ; mov Rd(target_reg as u8), BYTE value as _
                );
            }
            2 => {
                // Move to 32 bit register to zero extend.
                assemble!(block.assembler();
                    ; mov Rd(target_reg as u8), WORD value as _
                );
            }
            3..=4 => {
                // We need to move to 32 bit register here, since the 64 bit move sign extends?????
                assemble!(block.assembler();
                    ; mov Rd(target_reg as u8), DWORD value as _
                );
            }
            5..=8 => {
                assemble!(block.assembler();
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
                assemble!(self.block_mut()?.assembler();
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
            (self.load_slice_gpr(input_a, SliceType::Output), self.load_slice_gpr(input_b, SliceType::Input))
        } else if output == input_b {
            (self.load_slice_gpr(input_b, SliceType::Output), self.load_slice_gpr(input_a, SliceType::Input))
        } else {
            // Copy one of the operands to the output and then do the and operation
            self.emit_copy(output, input_a)?;
            (self.load_slice_gpr(output, SliceType::Output), self.load_slice_gpr(input_b, SliceType::Input))
        };
        Ok((target_gp, operand_gp))
    }

    /// Preserve all slices that are in registers that need to be preserved by the caller.
    /// Does not touch temporaries.
    pub(crate) fn preserve_registers_caller(&mut self) -> JITResult {
        // Iterate over the loaded slices and flush them to memory
        // TODO: Do not insert flush instructions for dead registers.
        for (register, allocation) in self.register_tracker.reset_gp().iter() {
            if let RegisterAllocation::Occupied(slice) = allocation {
                match register {
                    GPRegister::AX
                    | GPRegister::CX
                    | GPRegister::DX
                    | GPRegister::SI
                    | GPRegister::DI
                    | GPRegister::R8
                    | GPRegister::R9
                    | GPRegister::R10
                    | GPRegister::R11 => {
                        // preserved by caller.
                        self.emit_flush_register_to_slice(register, *slice)?;
                    }
                    GPRegister::BX
                    | GPRegister::SP
                    | GPRegister::BP
                    | GPRegister::R12
                    | GPRegister::R13
                    | GPRegister::R14
                    | GPRegister::R15 => {
                        // preserved by callee
                    }
                }
            } else {
                // Copy over old allocation (temps)
                self.register_tracker.gp[register] = *allocation;
            }
        }
        Ok(())
    }
}
