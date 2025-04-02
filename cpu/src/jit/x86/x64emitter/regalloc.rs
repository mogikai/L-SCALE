use crate::jit::api::{
    JITResult,
    JitError,
};
use crate::jit::assembler::MogiAssembler;
use crate::jit::code_buffer::CompiledBasicBlock;
use crate::jit::trace_jit;
use crate::jit::x86::registers::general_purpose::GPRegister;
use crate::jit::x86::registers::{
    RegAllocType,
    RegisterLoadInfo,
    RegisterTracker,
};
use crate::jit::x86::task_context::X86TaskContext;
use crate::jit::x86::x64emitter::X64Emitter;
use crate::jit::x86::x64emitter::slice_load::CopyExtendMode;
use crate::jit::x86::{
    X86MogiJit,
    assemble,
};
use dynasmrt::x64::X64Relocation;
use mogi_sleigh::spec::jir::values::JIRSlice;
use tracing::{
    trace,
    warn,
};

#[derive(Debug, Copy, Clone)]
pub(crate) enum RegAllocIntention {
    /// Allocate a temporary
    Temp,
    /// Allocate a slice. This slice is used for inputs and is not flushed to context.
    /// Spilling these slices is usually cheaper because they do not have to be written to context.
    /// The slice can be promoted to an [OutSlice](RegAllocIntention::OutSlice) which will cause
    /// it to get flushed to context upon de-allocation.
    Slice(JIRSlice),
    /// Allocate a slice that is flushed back to context
    OutSlice(JIRSlice),
    /// Reallocate a slice to a different gpr
    Realloc(JIRSlice),
}

#[derive(Debug, Copy, Clone)]
pub(crate) enum RegAllocHint {
    /// No hint. The allocator may choose the register freely.
    None,
    /// Allocate to the provided hint register. If an allocation already exists, the existing one is used.
    Allocate(GPRegister),
    /// Allocate to the provided hint register. If an allocation already exists, it is moved to the new register.
    Reallocate(GPRegister),
}

pub enum SliceType {
    Input,
    Output,
}

/// Describes what happens during register allocation, in case a spill is necessary.
pub(crate) enum SpillBehavior {
    /// Spill a register to make room for the new allocation.
    Spill,
    /// Fail the allocation.
    Fail,
}

impl X64Emitter<'_> {
    pub(super) fn allocate_gpr(
        &mut self,
        alloc_hint: RegAllocHint,
        alloc_intention: RegAllocIntention,
        spill_behavior: SpillBehavior,
    ) -> JITResult<RegisterLoadInfo<GPRegister>> {
        self.register_tracker.allocate_gpr(
            self.block.assembler(),
            Self::emit_flush_register_to_slice,
            alloc_hint,
            alloc_intention,
            spill_behavior,
        )
    }

    #[must_use]
    pub(super) fn temp_alloc(&mut self) -> GPRegister {
        let load_info = self
            .allocate_gpr(RegAllocHint::None, RegAllocIntention::Temp, SpillBehavior::Spill)
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
        match self.register_tracker.gp[gpr].ty {
            RegAllocType::Occupied(_) | RegAllocType::OccupiedDirty(_) => {
                self.move_slice_from_gp(gpr)?;
                self.register_tracker.gp[gpr].ty = RegAllocType::OccupiedTemp;
                Ok(gpr)
            }
            RegAllocType::Free => {
                self.register_tracker.gp[gpr].ty = RegAllocType::OccupiedTemp;
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
    /// CAVEAT: This operation may cause existing gp references to point to the wrong register.
    /// Moving should usually be done, before allocating any additional registers.
    /// Using this function directly should be avoided. Instead, consider using the `reserve`
    /// methods.
    #[inline]
    pub fn move_slice_from_gp(
        &mut self,
        gpr: GPRegister,
    ) -> JITResult {
        let slice = match self.register_tracker.gp[gpr].ty {
            RegAllocType::Free => {
                trace!("Moving slice from gp {gpr:?} to free register, but the register is already free.");
                // Nothing to do here, since the slot is already free.
                return Ok(());
            }
            RegAllocType::Reserved | RegAllocType::OccupiedTemp => {
                // We can only move from the gp if it contains a slice.
                trace_jit!("Cannot move slice from gp {gpr:?} since it contains {:?}", self.register_tracker.gp[gpr]);
                Err(JitError::CannotMoveNonSlice)?
            }
            RegAllocType::Occupied(slice) => slice,
            RegAllocType::OccupiedDirty(slice) => slice,
        };
        trace_jit!("Moving slice {slice:?} from gp {gpr:?} to free register");
        match self.allocate_gpr(RegAllocHint::None, RegAllocIntention::Realloc(slice), SpillBehavior::Fail) {
            Ok(RegisterLoadInfo::NeedRelocation { from, to }) => {
                // An empty register is available and we can move the slice to this gpr.
                self.copy_gpr_to_gpr(to, from, slice.size(), CopyExtendMode::None)?;
                Ok(())
            }
            Err(JitError::NoRegistersAvailable) => {
                // If we have no registers available to move to, we flush the slice to memory.
                Self::emit_flush_register_to_slice(self.block.assembler(), gpr, slice)
            }
            _ => panic!("Programming error"),
        }
    }

    /// Free a temporary register. If the register is not a temporary, this function does nothing.
    pub(super) fn temp_free(
        &mut self,
        gpr: GPRegister,
    ) -> JITResult {
        if let RegAllocType::OccupiedTemp = self.register_tracker.gp[gpr].ty {
            self.register_tracker.gp[gpr].ty = RegAllocType::Free;
        };
        Ok(())
    }

    pub(crate) fn emit_flush_slices(&mut self) -> JITResult {
        // Iterate over the loaded slices and flush them to memory
        for (register, alloc) in self.register_tracker.reset_gp().iter() {
            if let RegAllocType::OccupiedDirty(slice) = &alloc.ty {
                X64Emitter::emit_flush_register_to_slice(self.block.assembler(), register, *slice)?;
            }
        }
        Ok(())
    }

    /// Emit code to flush the provided [GPRegister] to the provided slice.
    pub(crate) fn emit_flush_register_to_slice(
        assembler: &mut MogiAssembler<X64Relocation>,
        register: GPRegister,
        slice: JIRSlice,
    ) -> JITResult {
        trace_jit!("Flushing slice {slice:?} which is currently loaded in {register:?}");
        let varnode_offset = X86TaskContext::slice_offset(slice);
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
    pub(super) fn allocate_slice_gpr(
        &mut self,
        slice: JIRSlice,
        slice_type: SliceType,
    ) -> GPRegister {
        let allocation_type = match slice_type {
            SliceType::Input => {
                warn!("Allocating input slice without loading. Is this intended?");
                RegAllocIntention::Slice(slice)
            }
            SliceType::Output => RegAllocIntention::OutSlice(slice),
        };
        let register = self
            .register_tracker
            .allocate_gpr(
                self.block.assembler(),
                Self::emit_flush_register_to_slice,
                RegAllocHint::None,
                allocation_type,
                SpillBehavior::Spill,
            )
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

    /// Gets the register associated with the provided slice, if it is allocated.
    /// This does not allocate and only serves as a lookup to check if a slice is already loaded.
    #[must_use]
    #[inline]
    pub(super) fn get_slice_gpr(
        &mut self,
        slice: JIRSlice,
    ) -> Option<GPRegister> {
        for (register, entry) in self.register_tracker.gp.iter() {
            match &entry.ty {
                RegAllocType::Occupied(entry) | RegAllocType::OccupiedDirty(entry) => {
                    if entry.group_id == slice.group_id {
                        return Some(register);
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// Return the temporary [GPRegister].
    #[inline]
    pub(super) fn get_temp_gpr(&self) -> GPRegister {
        GPRegister::AX
    }
}
