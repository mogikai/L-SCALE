use crate::jit::api::{
    JITResult,
    JitError,
};
use crate::jit::trace_jit;
use crate::jit::x86::emit::slice_load::CopyExtendMode;
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
use mogi_sleigh::spec::jir::values::JIRSlice;
use tracing::trace;

#[derive(Debug, Copy, Clone)]
pub(super) enum RegisterAllocationType {
    /// Allocate a temporary
    Temp,
    /// Allocate a slice. This slice is used for inputs and is not flushed to context.
    /// Spilling these slices is usually cheaper because they do not have to be written to context.
    /// The slice can be promoted to an [OutSlice](RegisterAllocationType::OutSlice) which will cause
    /// it to get flushed to context upon de-allocation.
    Slice(JIRSlice),
    /// Allocate a slice that is flushed back to context
    OutSlice(JIRSlice),
    /// Reallocate a slice to a different gpr
    Realloc(JIRSlice),
}

pub enum SliceType {
    Input,
    Output,
}

/// Describes what happens during register allocation, in case a spill is necessary.
pub(super) enum SpillBehavior {
    /// Spill a register to make room for the new allocation.
    Spill,
    /// Fail the allocation.
    Fail,
}

impl X86MogiJit {
    #[must_use]
    pub(super) fn temp_alloc(&mut self) -> GPRegister {
        let load_info = self
            .allocate_gp(None, RegisterAllocationType::Temp, SpillBehavior::Spill)
            .expect("Since spill was provided, this must not fail");
        // It doesn't matter if the register is loaded or unloaded, since this is a
        // temporary, and it has no backing storage
        match load_info {
            RegisterLoadInfo::Unloaded(gpr) => gpr,
            RegisterLoadInfo::Loaded(gpr) => gpr,
            RegisterLoadInfo::NeedRelocation { .. } => {
                panic!("This case cannot happen when allocating a temporary")
            }
        }
    }

    /// Allocate the provided gp register as temporary and flush its contents to memory if needed.
    pub(super) fn reserve_gp_as_temp(
        &mut self,
        gpr: GPRegister,
    ) -> JITResult<GPRegister> {
        let allocation = self.register_tracker.gp[gpr];
        match allocation {
            RegisterAllocation::Occupied(slice) => {
                self.move_slice_from_gp(gpr)?;
                self.register_tracker.gp[gpr] = RegisterAllocation::OccupiedTemp;
                Ok(gpr)
            }
            RegisterAllocation::Free => {
                self.register_tracker.gp[gpr] = RegisterAllocation::OccupiedTemp;
                Ok(gpr)
            }
            _ => {
                // Reserved or already allocated temps cannot be used.
                Err(JitError::AlreadyAllocated)
            }
        }
    }

    /// Moves the provided slice into a different free register. If no registers are available,
    /// this will spill the slice into backing memory.
    #[inline]
    pub fn move_slice_from_gp(
        &mut self,
        gpr: GPRegister,
    ) -> JITResult {
        let slice = match self.register_tracker.gp[gpr] {
            RegisterAllocation::Free => {
                // Nothing to do here, since the slot is already free.
                return Ok(());
            }
            RegisterAllocation::Reserved | RegisterAllocation::OccupiedTemp => {
                // We can only move from the gp if it contains a slice.
                println!("Cannot move slice from gp {gpr:?} since it contains {:?}", self.register_tracker.gp[gpr]);
                Err(JitError::CannotMoveNonSlice)?
            }
            RegisterAllocation::Occupied(slice) => slice,
            RegisterAllocation::OccupiedDirty(slice) => slice,
        };
        match self.allocate_gp(None, RegisterAllocationType::Realloc(slice), SpillBehavior::Fail) {
            Ok(RegisterLoadInfo::NeedRelocation { from, to }) => {
                // An empty register is available and we can move the slice to this gpr.
                self.copy_gpr_to_gpr(to, from, slice.size(), CopyExtendMode::None)?;
                Ok(())
            }
            Err(JitError::NoRegistersAvailable) => {
                // If we have no registers available to move to, we flush the slice to memory.
                self.emit_flush_register_to_slice(gpr, slice)
            }
            _ => panic!("Programming error"),
        }
    }

    pub(super) fn temp_free(
        &mut self,
        gpr: GPRegister,
    ) -> JITResult {
        let RegisterAllocation::OccupiedTemp = self.register_tracker.gp[gpr] else {
            Err(JitError::InvalidTempFree)?
        };
        self.register_tracker.gp[gpr] = RegisterAllocation::Free;
        Ok(())
    }

    /// Emit code to flush the provided [GPRegister] to the provided slice.
    pub(super) fn emit_flush_register_to_slice(
        &mut self,
        register: GPRegister,
        slice: JIRSlice,
    ) -> JITResult {
        let varnode_offset = X86TaskContext::varnode_offset(slice);
        let assembler = &mut self.block_mut()?.assembler();
        // Emit the instruction that loads the varnode from the specified space in the context.
        match slice.size() {
            1 => {
                assemble!(assembler;
                    ; mov BYTE [cpu_context + varnode_offset as _], Rb(register as u8)
                );
            }
            2 => {
                assemble!(assembler;
                    ; mov WORD [cpu_context + varnode_offset as _], Rw(register as u8)
                );
            }
            4 => {
                assemble!(assembler;
                    ; mov DWORD [cpu_context + varnode_offset as _], Rd(register as u8)
                );
            }
            8 => {
                assemble!(assembler;
                    ; mov QWORD [cpu_context + varnode_offset as _], Rq(register as u8)
                );
            }
            _ => {
                panic!("Unsupported store size")
            }
        }
        Ok(())
    }

    /// Gets the register associated with the provided slice.
    /// This allocates a register to the slice, if it does not have one yet.
    /// This may also spill registers into memory, if all registers are occupied.
    /// Since this operation does not perform a load of the underlying value,
    /// it should only be used to get registers that are written to, since the value
    /// is irrelevant for those.
    #[must_use]
    #[inline]
    pub(super) fn get_slice_gp(
        &mut self,
        slice: JIRSlice,
        slice_type: SliceType,
    ) -> GPRegister {
        let allocation_type = match slice_type {
            SliceType::Input => RegisterAllocationType::Slice(slice),
            SliceType::Output => RegisterAllocationType::OutSlice(slice),
        };
        let register = self
            .allocate_gp(None, allocation_type, SpillBehavior::Spill)
            .expect("Since spill was provided, the register allocation must not fail.");
        match register {
            RegisterLoadInfo::Unloaded(register) => {
                trace_jit!("Allocated slice {slice:?} to GP register {register:?} without loading");
                register
            }
            RegisterLoadInfo::Loaded(register) => {
                trace_jit!("Allocated slice {slice:?} to GP register {register:?} without loading");
                register
            }
            RegisterLoadInfo::NeedRelocation { .. } => panic!("This case cannot happen when allocating a slice"),
        }
    }

    /// Allocate a gp register for use as a temporary or slice, depending on `alloc_type`.
    /// If a slice is already loaded, the corresponding GPR is returned.
    /// If a slice has not been loaded previously, a free GPR is returned, but no load is performed.
    /// The [RegisterLoadInfo] indicates, whether the GPR is loaded or not.
    /// Unloaded GPRs can be used if the slice appears as output, since we will overwrite the value anyway.
    pub(super) fn allocate_gp(
        &mut self,
        hint: Option<GPRegister>,
        alloc_type: RegisterAllocationType,
        spill_behavior: SpillBehavior,
    ) -> JITResult<RegisterLoadInfo<GPRegister>> {
        let mut free_gpr = None;
        let mut old_gpr = None;
        // TODO: We need to check if two slices refer to the same memory.
        //       This needs to be done because there might be a slice to e.g. EAX loaded, but another
        //       instruction refers to RAX. The RAX load then needs to be performed from the loaded
        //       EAX, instead of from the space, or it needs to be flushed first.
        // Search for existing register, and otherwise take the first available GP register.
        for (register, entry) in self.register_tracker.gp.iter() {
            match entry {
                RegisterAllocation::Free => {
                    if let Some(hint) = hint
                        && hint == register
                    {
                        // If the hinted register is free, we set it as the chosen register
                        // even if another free register was already found.
                        free_gpr = Some(register);
                    }
                    if free_gpr.is_none() {
                        // Save the first free register
                        free_gpr = Some(register)
                    }
                }
                RegisterAllocation::Reserved | RegisterAllocation::OccupiedTemp => {}
                RegisterAllocation::Occupied(entry) | RegisterAllocation::OccupiedDirty(entry) => match alloc_type {
                    RegisterAllocationType::Slice(slice) if entry.varnode_id == slice.varnode_id => {
                        return Ok(RegisterLoadInfo::Loaded(register));
                    }
                    RegisterAllocationType::OutSlice(slice) if entry.varnode_id == slice.varnode_id => {
                        old_gpr = Some(register);
                        break;
                    }
                    RegisterAllocationType::Realloc(slice) if entry.varnode_id == slice.varnode_id => {
                        old_gpr = Some(register);
                    }
                    _ => {}
                },
            }
        }
        if let Some(free_gpr) = free_gpr {
            match alloc_type {
                RegisterAllocationType::Temp => {
                    self.register_tracker.gp[free_gpr] = RegisterAllocation::OccupiedTemp;
                    Ok(RegisterLoadInfo::Loaded(free_gpr))
                }
                RegisterAllocationType::Slice(slice) => {
                    self.register_tracker.gp[free_gpr] = RegisterAllocation::Occupied(slice);
                    Ok(RegisterLoadInfo::Unloaded(free_gpr))
                }
                RegisterAllocationType::OutSlice(slice) => {
                    let Some(old_gpr) = old_gpr else {
                        // The out slice is not loaded yet, we need to load it now.
                        self.register_tracker.gp[free_gpr] = RegisterAllocation::OccupiedDirty(slice);
                        return Ok(RegisterLoadInfo::Unloaded(free_gpr));
                    };
                    // Promote old gpr to occupied dirty such that it is flushed correctly.
                    self.register_tracker.gp[old_gpr] = RegisterAllocation::OccupiedDirty(slice);
                    Ok(RegisterLoadInfo::Loaded(old_gpr))
                }
                RegisterAllocationType::Realloc(slice) => {
                    let Some(old_gpr) = old_gpr else {
                        Err(JitError::SliceNotLoaded)?
                    };
                    self.register_tracker.gp[old_gpr] = RegisterAllocation::Free;
                    self.register_tracker.gp[free_gpr] = RegisterAllocation::Occupied(slice);
                    Ok(RegisterLoadInfo::NeedRelocation {
                        from: old_gpr,
                        to: free_gpr,
                    })
                }
            }
        } else {
            if let RegisterAllocationType::OutSlice(slice) = alloc_type {
                let Some(old_gpr) = old_gpr else {
                    Err(JitError::SliceNotLoaded)?
                };
                self.register_tracker.gp[old_gpr] = RegisterAllocation::OccupiedDirty(slice);
                return Ok(RegisterLoadInfo::Loaded(old_gpr));
            }
            match spill_behavior {
                SpillBehavior::Spill => todo!("Handle register spill"),
                SpillBehavior::Fail => Err(JitError::NoRegistersAvailable),
            }
        }
    }

    /// Return the temporary [GPRegister].
    #[inline]
    pub(super) fn get_temp_gpr(&self) -> GPRegister {
        GPRegister::AX
    }
}
