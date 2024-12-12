use crate::jit::api::{
    JITResult,
    JitError,
};
use crate::jit::x86::registers::{
    GPRegister,
    RegisterAllocation,
    RegisterLoadInfo,
};
use crate::jit::x86::{
    X86MogiJit,
    X86TaskContext,
    assemble,
};
use mogi_sleigh::spec::jir::common::JIRSize;
use mogi_sleigh::spec::jir::values::JIRSlice;
use tracing::trace;

pub(super) enum RegisterAllocationType {
    /// Allocate a temporary
    Temp,
    /// Allocate a slice
    Slice(JIRSlice),
    /// Reallocate a slice to a different gpr
    Realloc(JIRSlice),
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
    ) -> JITResult {
        let allocation = self.register_tracker.gp[gpr];
        match allocation {
            RegisterAllocation::Occupied(slice) => {
                self.move_slice_from_gp(gpr)?;
                self.register_tracker.gp[gpr] = RegisterAllocation::OccupiedTemp;
                Ok(())
            }
            RegisterAllocation::Free => {
                self.register_tracker.gp[gpr] = RegisterAllocation::OccupiedTemp;
                Ok(())
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
        let RegisterAllocation::Occupied(slice) = self.register_tracker.gp[gpr] else {
            // We can only move from the gp if it contains a slice.
            Err(JitError::CannotMoveNonSlice)?
        };
        match self.allocate_gp(None, RegisterAllocationType::Realloc(slice), SpillBehavior::Fail) {
            Ok(RegisterLoadInfo::NeedRelocation { from, to }) => {
                // An empty register is available and we can move the slice to this gpr.
                self.copy_gpr_to_gpr(to, from, slice.size())?;
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
        let assembler = &mut self.block_mut()?.assembler;
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
    ) -> GPRegister {
        let register = self
            .allocate_gp(None, RegisterAllocationType::Slice(slice), SpillBehavior::Spill)
            .expect("Since spill was provided, the register allocation must not fail.");
        match register {
            RegisterLoadInfo::Unloaded(register) => register,
            RegisterLoadInfo::Loaded(register) => register,
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
                RegisterAllocation::Occupied(entry) => match alloc_type {
                    RegisterAllocationType::Slice(slice) if *entry == slice => {
                        trace!("Slice {slice:?} is already loaded in GP register {register:?}");
                        return Ok(RegisterLoadInfo::Loaded(register));
                    }
                    RegisterAllocationType::Realloc(slice) if *entry == slice => {
                        old_gpr = Some(register);
                    }
                    _ => {}
                },
            }
        }
        if let Some(free_gpr) = free_gpr {
            match alloc_type {
                RegisterAllocationType::Temp => Ok(RegisterLoadInfo::Loaded(free_gpr)),
                RegisterAllocationType::Slice(slice) => {
                    trace!("Allocating new GP register {free_gpr:?} for slice {slice:?}");
                    self.register_tracker.gp[free_gpr] = RegisterAllocation::Occupied(slice);
                    Ok(RegisterLoadInfo::Unloaded(free_gpr))
                }
                RegisterAllocationType::Realloc(slice) => {
                    let Some(old_gpr) = old_gpr else {
                        Err(JitError::SliceNotLoaded)?
                    };
                    trace!("Reallocating slice {slice:?} from gpr {old_gpr:?} to {free_gpr:?}...");
                    self.register_tracker.gp[old_gpr] = RegisterAllocation::Free;
                    self.register_tracker.gp[free_gpr] = RegisterAllocation::Occupied(slice);
                    Ok(RegisterLoadInfo::NeedRelocation {
                        from: old_gpr,
                        to: free_gpr,
                    })
                }
            }
        } else {
            match spill_behavior {
                SpillBehavior::Spill => todo!("Handle register spill"),
                SpillBehavior::Fail => Err(JitError::NoRegistersAvailable),
            }
        }
    }

    /// Return the temporary [GPRegister].
    #[inline]
    pub(super) fn get_temp_gpr(&self) -> GPRegister {
        GPRegister::A
    }
}
