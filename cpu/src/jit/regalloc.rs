//!
//!
//! Register allocation:
//!
//! Slices can be allocated as input or output arguments. Output arguments will be written to backing-memory
//! at some point, whereas Input arguments will be loaded from backing memory at some point.
//!
//! Problematic: Different sized slice inputs.
//! What happens for instructions like a:8 = zext(a:4)?
//! In this case there will be two operand requests: One for writing a:8 and one for reading a:4.
//! We need to allocate the same register for these two.
//!
//!

mod instruction;
mod lru;
pub mod requirements;
pub mod result;

use crate::jit::api::{
    JITResult,
    JitError,
};
use crate::jit::host::register::{
    HostReg,
    HostRegMap,
    HostRegSet,
    RegClass,
};
use crate::jit::regalloc::lru::Lru;
use crate::jit::regalloc::requirements::{
    OperandConstraint,
    OperandKind,
    OperandPos,
    OperandRequest,
    TempRequest,
};
use crate::jit::regalloc::result::Allocation;
use crate::jit::trace_jit;
use mogi_sleigh::spec::jir::basic_block::{
    InstId,
    TargetBasicBlock,
};
use mogi_sleigh::spec::jir::jir_block::JIRBlockId;
use mogi_sleigh::spec::jir::values::JIRSlice;
use requirements::EmitterAllocRequirements;
use result::AllocatorResult;
use std::ops::{
    Index,
    IndexMut,
};
use strum::EnumCount;

#[derive(Debug)]
pub struct RegAllocHostConstraints {
    all_available_registers: HostRegSet,
    preferred_registers: HostRegSet,
    non_preferred_registers: HostRegSet,
    /// Some instructions need a scratch register. This register can be used
    /// by any instruction and any directive, but it gives no guarantees
    /// to its contents before or after the corresponding instruction or directive.
    /// This register can never be allocated to a slice.
    /// Each class has one scratch register.
    scratch_register: [HostReg; RegClass::COUNT],
}

impl RegAllocHostConstraints {
    pub const fn new(
        preferred_registers: HostRegSet,
        non_preferred_registers: HostRegSet,
    ) -> Self {
        assert!(!preferred_registers.intersects(non_preferred_registers));
        let mut all_available_registers = preferred_registers;
        all_available_registers.union_from(non_preferred_registers);
        RegAllocHostConstraints {
            all_available_registers,
            preferred_registers,
            non_preferred_registers,
            scratch_register: [HostReg::invalid(); RegClass::COUNT],
        }
    }

    /// Add `reg` as scratch register.
    /// This automatically removes it from the preferred and non-preferred register sets.
    pub const fn with_scratch(
        mut self,
        reg: HostReg,
    ) -> Self {
        self.preferred_registers.remove(reg);
        self.non_preferred_registers.remove(reg);
        self.scratch_register[reg.class() as usize] = reg;
        self
    }
}

struct Lrus {
    lrus: [Lru; RegClass::COUNT],
}

impl Index<RegClass> for Lrus {
    type Output = Lru;

    fn index(
        &self,
        index: RegClass,
    ) -> &Self::Output {
        &self.lrus[index as usize]
    }
}

impl IndexMut<RegClass> for Lrus {
    fn index_mut(
        &mut self,
        index: RegClass,
    ) -> &mut Self::Output {
        &mut self.lrus[index as usize]
    }
}

impl Lrus {
    pub fn new(host_constraints: &RegAllocHostConstraints) -> Self {
        // TODO: Would it make sense to abe able to specify an exact initial lru state?
        let initial_lru_order_int = host_constraints
            .preferred_registers
            .iter_class(RegClass::Int)
            .chain(
                host_constraints
                    .non_preferred_registers
                    .iter_class(RegClass::Int),
            )
            .collect::<Vec<_>>();
        let initial_lru_order_vec = host_constraints
            .preferred_registers
            .iter_class(RegClass::Vector)
            .chain(
                host_constraints
                    .non_preferred_registers
                    .iter_class(RegClass::Vector),
            )
            .collect::<Vec<_>>();
        let initial_lru_order_float = host_constraints
            .preferred_registers
            .iter_class(RegClass::Float)
            .chain(
                host_constraints
                    .non_preferred_registers
                    .iter_class(RegClass::Float),
            )
            .collect::<Vec<_>>();
        // TODO: can we somehow create this array in a way that depends on the order of the enum variants, instead
        //       of just assuming that they are ordered in this way?
        Self {
            lrus: [
                Lru::new(RegClass::Int, &initial_lru_order_int),
                Lru::new(RegClass::Float, &initial_lru_order_float),
                Lru::new(RegClass::Vector, &initial_lru_order_vec),
            ],
        }
    }
}

pub struct RegisterAllocator<'a, E> {
    /// The target basic block. It contains the instructions and JirBlocks that the allocator performs allocations for.
    target_block: &'a TargetBasicBlock,

    /// The emitter. It specifies the operand and register requirements for each instruction.
    /// The emitter will later use the allocation result to emit the actual assembly.
    emitter_requirements: &'a E,

    allocations: AllocatorResult,

    /// Maps the [HostReg]
    loaded_slices: HostRegMap<JIRSlice>,

    /// Keeps track of registers that have been written to previously.
    /// This info is kept in reverse direction.
    /// Registers that are already in this set do not need another spill instruction emitted, since
    /// they have already been written.
    written_regs: HostRegSet,
    /// Keeps track of registers that have been read previously.
    /// This info is kept in reverse direction.
    /// Any register that has been read that is in this set needs to be loaded at the start of a block.
    read_regs: HostRegSet,

    /// TODO: Add the appropriate env constraints. We probably need to be able to exclude some registers for example.
    ///       It might also make sense to define the set of usable registers this way.
    host_constraints: RegAllocHostConstraints,

    /// Keeps track of the available registers for an instruction.
    /// This set of registers is reset for each instruction according to
    /// the instructions' and environments' constraints.
    available_registers: HostRegSet,

    /// Keeps track of the fixed registers that need to be allocated per instruction.
    /// This is needed to make sure that no existing allocations that by chance
    /// are already in the fixed register slot are kept there.
    /// If this case is encountered, then the allocation will be moved to a free register.
    fixed_regs: HostRegSet,

    /// Keeps track of which register was least recently used, such that we can spill
    /// this register if needed.
    lrus: Lrus,
}

/// A struct to keep track of the registers that are available before and after an instruction.
#[derive(Debug)]
struct AvailableRegisters {
    before: HostRegSet,
    after: HostRegSet,
}

impl<'a, E> RegisterAllocator<'a, E>
where
    E: EmitterAllocRequirements,
{
    /// Create a new register allocator for the target basic block.
    pub fn new(
        translation_unit: &'a TargetBasicBlock,
        emitter_requirements: &'a E,
        host_constraints: RegAllocHostConstraints,
    ) -> Self {
        let lrus = Lrus::new(&host_constraints);
        RegisterAllocator::<'a, E> {
            target_block: translation_unit,
            emitter_requirements,
            allocations: AllocatorResult::new(
                translation_unit,
                emitter_requirements,
                host_constraints.scratch_register,
            ),
            loaded_slices: Default::default(),
            written_regs: HostRegSet::empty(),
            read_regs: HostRegSet::empty(),
            available_registers: host_constraints.all_available_registers,
            host_constraints,
            fixed_regs: HostRegSet::empty(),
            lrus,
        }
    }

    pub fn run(mut self) -> JITResult<AllocatorResult> {
        trace_jit!("Running register allocator");
        // Iterate in reverse over all blocks.
        for block in self.target_block.jir_block_ids().rev() {
            self.alloc_block(block)?;
        }
        Ok(self.allocations)
    }

    /// Makes sure that block inputs, i.e. every slice that is live at the start of a block,
    /// are loaded.
    fn load_block_inputs(&mut self) -> JITResult {
        trace_jit!("Loading block inputs");
        for (reg, slice) in self.loaded_slices.drain() {
            // We are at the start of a block. Make sure that the slice is loaded in the
            // register as specified.
            // TODO: We need to track whether the register is only ever written to and omit loads
            //       to these registers here, and probably at some other locations as well.
            if self.read_regs.contains(reg) {
                self.allocations
                    .load_spill_directives
                    .add_load_before(reg, slice);
            }
        }
        self.read_regs = HostRegSet::empty();
        self.written_regs = HostRegSet::empty();
        self.allocations
            .load_spill_directives
            .update_current_inst_directives_end();
        Ok(())
    }

    fn alloc_block(
        &mut self,
        block: JIRBlockId,
    ) -> JITResult {
        for inst in self.target_block.jir_block_inst_ids(block).rev() {
            self.alloc_inst(inst)?;
        }
        self.load_block_inputs()?;
        // TODO: check block inputs and make sure they are allocated/loaded here.
        // We are iterating backwards, so we are at the start of a block here,
        // which means we have to load any varnodes that are live in the following instructions.
        Ok(())
    }

    fn reset_available_host_registers(&mut self) {
        self.available_registers = self.host_constraints.all_available_registers;
        self.fixed_regs = HostRegSet::empty();
    }

    fn alloc_inst(
        &mut self,
        inst: InstId,
    ) -> JITResult {
        // Start the directives section for this instruction.
        self.allocations
            .load_spill_directives
            .start_inst_directives(inst)?;
        // At the start we set the available registers to all configured registers.
        // We then subsequently remove all registers that are used by clobbers, fixed
        // register operands, or are already allocated.
        self.reset_available_host_registers();

        let clobbers = self.emitter_requirements.clobbers(inst);
        let operands = self.emitter_requirements.operands(inst);
        let required_temps = self.emitter_requirements.required_temps(inst);
        if clobbers.is_empty() && operands.all().len() == 0 {
            trace_jit!("No operands or clobbers for {inst}, skipping allocation");
            return Ok(());
        }
        trace_jit!("");
        trace_jit!("Allocating registers for {inst}. It is clobbering {clobbers} and requesting {operands:?}");
        trace_jit!("Available registers at the start:\n{}", self.available_registers,);

        for (_, operand) in operands.fixed() {
            self.reserve_reg_for_fixed_operand(operand)?;
        }

        for clobber in clobbers {
            self.clear_clobber(clobber);
        }
        self.allocations.clobbers[inst].union_from(clobbers);

        trace_jit!("Available registers after fixed reg and clobber constraints:\n{}", self.available_registers);

        for (op_id, operand) in operands.write_fixed() {
            let allocation = self.alloc_operand(operand)?;
            trace_jit!("Allocated fixed reg operand {op_id}|{operand:?} for instruction {inst} as {allocation:?}");
            self.allocations.set_allocation(inst, op_id, allocation);
        }

        for (op_id, operand) in operands.write_non_fixed() {
            let allocation = self.alloc_operand(operand)?;
            trace_jit!("Allocated operand {op_id}|{operand:?} for instruction {inst} as {allocation:?}");
            self.allocations.set_allocation(inst, op_id, allocation);
        }

        for (op_id, operand) in operands.read_fixed() {
            let allocation = self.alloc_operand(operand)?;
            trace_jit!("Allocated fixed reg operand {op_id}|{operand:?} for instruction {inst} as {allocation:?}");
            self.allocations.set_allocation(inst, op_id, allocation);
        }

        for (op_id, operand) in operands.read_non_fixed() {
            let allocation = self.alloc_operand(operand)?;
            trace_jit!("Allocated operand {op_id}|{operand:?} for instruction {inst} as {allocation:?}");
            self.allocations.set_allocation(inst, op_id, allocation);
        }

        let temp_regs = self.allocate_temp_regs(required_temps)?;
        // We add the temp regs to the clobbers to signal that these may not contain any valid data.
        self.allocations.clobbers[inst].union_from(temp_regs);
        self.allocations.temp_allocations[inst] = temp_regs;

        // TODO: We might need to track the number of uses per allocation, such that we can
        //       replace register allocations with memory allocations, in case the value is
        //       only used a certain number of times.

        // Any read operands need to be allocated. No load is necessary here. We simply allocate.
        // Once the slice is deallocated at an earlier instruction, the load will be inserted.

        // Any Write operands need to be allocated, and a write operation needs to be inserted, if the
        // slice is not allocated as write yet.

        // TODO: For now, we most likely just want to spill everything at the end of each block, and assume that
        //       nothing is loaded at the start of each block, for simplicities sake.
        //       In the future, we can maybe put the live out and live in registers into some kind of resolver
        //       That then tries to remap the corresponding registers, instead of flushing them.

        // TODO:
        // if inst.is_branch()
        //     spill all for diverging branches
        //     keep everything loaded for main path (either fallthrough or unconditional jmp)
        trace_jit!("Available registers after allocation:\n{}", self.available_registers);
        Ok(())
    }

    /// Checks if the slice group is already loaded with an overlap, and returns the host reg and loaded slice
    /// if it exists.
    fn get_loaded_slice_group(
        &self,
        operand: &OperandRequest,
    ) -> Option<(HostReg, JIRSlice)> {
        self.loaded_slices
            .iter_class(operand.class)
            .find_map(|(reg, loaded_slice)| {
                if loaded_slice.same_group(&operand.slice)
                    && loaded_slice.space_id == operand.slice.space_id
                    && loaded_slice.overlaps(&operand.slice)
                {
                    Some((reg, *loaded_slice))
                } else {
                    None
                }
            })
    }

    fn find_suitable_reg(
        &mut self,
        class: RegClass,
    ) -> JITResult<HostReg> {
        // TODO: Use lru cache to pick least recently used register to allocate to
        let mut available_regs = self.available_registers;
        let Some(reg) = self.lrus[class].last(available_regs) else {
            todo!("Error: No registers available")
        };
        Ok(reg)
    }

    fn allocate_temp_regs(
        &mut self,
        request: TempRequest,
    ) -> JITResult<HostRegSet> {
        trace_jit!("Allocating temps: {request:?}");
        let mut temp_regs = HostRegSet::empty();

        for reg in self
            .available_registers
            .iter_class(RegClass::Int)
            .take(request.requested_int() as usize)
        {
            trace_jit!("Allocating temp int register {reg}");
            self.available_registers.remove(reg);
            self.unload_reg(reg);
            temp_regs.add(reg);
        }

        for reg in self
            .available_registers
            .iter_class(RegClass::Float)
            .take(request.requested_float() as usize)
        {
            trace_jit!("Allocating temp float register {reg}");
            self.available_registers.remove(reg);
            self.unload_reg(reg);
            temp_regs.add(reg);
        }

        for reg in self
            .available_registers
            .iter_class(RegClass::Vector)
            .take(request.requested_vector() as usize)
        {
            trace_jit!("Allocating temp vector register {reg}");
            self.available_registers.remove(reg);
            self.unload_reg(reg);
            temp_regs.add(reg);
        }
        Ok(temp_regs)
    }

    /// Allocates the provided operand.
    /// The operand will either reside in an allocated register or is kept in memory.
    /// Returns an error if no operand could be allocated.
    fn alloc_unloaded_operand(
        &mut self,
        operand: &OperandRequest,
    ) -> JITResult<Allocation> {
        // TODO: Pick a register where it "hurts" the least to spill it.
        //       Most likely this will be done with an LRU register or operand list.

        // TODO: Potentially return a memory allocation if the operand is used infrequently.
        let reg = match operand.constraint {
            OperandConstraint::Any | OperandConstraint::Reg => self.find_suitable_reg(operand.class)?,
            OperandConstraint::FixedReg(reg) => reg,
        };

        self.available_registers.remove(reg);

        // Spill the register if it is occupied by a slice, since we need to make sure it is loaded
        // with the correct value after the instruction finishs.
        self.unload_reg(reg);
        self.loaded_slices.insert(reg, operand.slice);
        Ok(Allocation::Register(reg))
    }

    /// Updates lru cache and marks it as written read in the respective tracking sets.
    /// If the register is written to, it is spilled if this is the first write for the currently loaded slice.
    fn mark_and_spill(
        &mut self,
        reg: HostReg,
        operand: &OperandRequest,
    ) {
        match operand.kind {
            OperandKind::Write => {
                self.spill_reg(reg, operand.slice);
                self.fixed_regs.remove(reg);
                self.written_regs.add(reg);
                // Remove from read, since we write to the register. No fetches have to be performed beforehand
                // If another operand request also reads from the same slice, the read will be inserted back.
                self.read_regs.remove(reg);
            }
            OperandKind::Read => {
                self.read_regs.add(reg);
            }
        }
        self.lrus[reg.class()].poke(reg);
    }

    /// Makes sure that the already loaded slice group corresponds to the operand request.
    /// The slice group needs to be adjusted if the offset or length do not match.
    /// Also, the slice needs to be moved, if the operand request contains a fixed reg constraint.
    fn ensure_valid_reg_allocation(
        &mut self,
        reg: HostReg,
        existing_slice: JIRSlice,
        operand: &OperandRequest,
    ) -> JITResult<HostReg> {
        trace_jit!(
            "Slice group is already loaded in register {reg} with slice {existing_slice:?}. Trying to also fit {:?}",
            operand.slice
        );
        // if we have an exact match, we can return the register if it is not fixed
        let reg = if let OperandConstraint::FixedReg(fixed_reg) = operand.constraint {
            if existing_slice == operand.slice {
                // If the fixed constraint specifies a different register, we have to move the allocation there
                if reg != fixed_reg {
                    // Move the allocation for any following instructions (in forward direction)
                    // into reg.
                    self.move_reg(reg, fixed_reg, existing_slice);
                }
                // The slices match and are already loaded in the correct register. Nothing to do here.
            } else {
                // We first check if the allocation is in the wrong register and insert the move first.
                // The move will later be the last directive to be executed.
                if reg != fixed_reg {
                    // Move the allocation for any following instructions (in forward direction)
                    // into reg.
                    self.move_reg(reg, fixed_reg, existing_slice);
                }
                // We can now adjust the slice. The adjust directive will later be executed before the move.
                self.adjust_reg(fixed_reg, existing_slice, operand.slice);
            }
            fixed_reg
        } else {
            let reg = if self.fixed_regs.contains(reg) {
                // The register was already reserved for a fixed register allocation.
                let new_reg = self.find_suitable_reg(operand.class)?;
                self.unload_reg(new_reg);
                self.move_reg(reg, new_reg, existing_slice);
                new_reg
            } else {
                reg
            };
            if existing_slice != operand.slice {
                if !self.available_registers.contains(reg) {
                    if existing_slice.offset != operand.slice.offset {
                        // We already allocated something to `reg` and it has a different offset.
                        todo!("Different offset aliasing in the same instruction currently unsupported")
                    } else {
                        if let OperandKind::Write = operand.kind {
                            todo!()
                        } else {
                            // Offsets are the same and we already allocated a different size for this instruction.
                            // Overwrite the old allocation if the new one has larger size.
                            // This way we request a larger slice than we need, which is fine because we only use
                            // the smaller size.
                            if operand.slice.size > existing_slice.size {
                                self.loaded_slices.insert(reg, operand.slice);
                            }
                        }
                        // And remove it from the available registers for current allocations.
                    }
                } else {
                    // The register has not been allocated for this instruction yet. Insert an adjust directive.
                    // After the instruction is finished, adjust the slice to the one that was previously expected.
                    // (that is expected next when viewing in forward direction)
                    self.adjust_reg(reg, existing_slice, operand.slice);
                }
            }
            // Exact match and no register requirement. Everything is ok.
            reg
        };
        self.available_registers.remove(reg);
        Ok(reg)
    }

    fn alloc_operand(
        &mut self,
        operand: &OperandRequest,
    ) -> JITResult<Allocation> {
        trace_jit!("Written regs: {}", self.written_regs);
        let allocation = if let Some((reg, existing_slice)) = self.get_loaded_slice_group(operand) {
            Allocation::Register(self.ensure_valid_reg_allocation(reg, existing_slice, operand)?)
        } else {
            // Slice group is not in any register. We can allocate one from the free regs.
            self.alloc_unloaded_operand(operand)?
        };
        if let Allocation::Register(reg) = allocation {
            self.mark_and_spill(reg, operand);
        }
        trace_jit!("Written regs: {}", self.written_regs);
        Ok(allocation)
    }

    fn reserve_reg_for_fixed_operand(
        &mut self,
        operand: &OperandRequest,
    ) -> JITResult {
        let OperandConstraint::FixedReg(target_reg) = operand.constraint else {
            unreachable!()
        };
        trace_jit!("Reserving register {target_reg} for fixed operand");

        // Check if the register is free, and remove it from the available set.
        if !self.available_registers.contains(target_reg) {
            todo!("Allocation error: Tried to allocate a fixed register for an unavailable register")
        }
        self.available_registers.remove(target_reg);
        self.fixed_regs.add(target_reg);
        Ok(())
    }

    #[inline]
    fn clear_clobber(
        &mut self,
        clobber: HostReg,
    ) {
        trace_jit!("Clearing clobber {clobber}");
        if !self.available_registers.contains(clobber) {
            todo!(
                "Error: Clobbering already removed register.
                This likely means that a clobber overlaps with a fixed register allocation.
                This must be avoided.
                It could also mean that the register is generally not available, since it is reserved."
            )
        }
        self.unload_reg(clobber);
        self.available_registers.remove(clobber);
    }

    /// Moves the allocation such that for any previous instructions (when viewing in reverse)
    /// the slice will be in `previous_reg`
    fn move_reg(
        &mut self,
        previous_reg: HostReg,
        current_reg: HostReg,
        slice: JIRSlice,
    ) {
        trace_jit!("Allocation is wanted in {current_reg}, but is already in {previous_reg}. Moving allocation");
        // We add a move such that the allocation will be in `previous_reg` after the instruction.
        // This is necessary, since any following instructions (in forward order)
        // are already allocated to the register `previous_reg`.
        self.allocations
            .load_spill_directives
            .add_move_after(previous_reg, current_reg, slice.size);
        // And update the loaded slices to point to current_reg
        // We update this for the reverse direction, since the loaded_slices map is relevant for not yet
        // allocated slices.
        self.loaded_slices.r#move(current_reg, previous_reg);
        self.read_regs.r#move(current_reg, previous_reg);
        self.written_regs.r#move(current_reg, previous_reg);
    }

    fn adjust_reg(
        &mut self,
        reg: HostReg,
        old_slice: JIRSlice,
        new_slice: JIRSlice,
    ) {
        trace_jit!("Adjusting allocation of register {reg} from {old_slice} to {new_slice}");
        // After the instruction is finished, adjust the slice to the one that was previously expected.
        // (that is expected next when viewing in forward direction)
        self.allocations
            .load_spill_directives
            .add_adjust_after(reg, new_slice, old_slice);
        // Insert the new slice as loaded in the register.
        self.loaded_slices.insert(reg, new_slice);
    }

    /// Unload the register, thus making sure that the register is loaded for all previous instructions,
    /// but usable for any future instruction for now.
    #[inline]
    fn unload_reg(
        &mut self,
        reg: HostReg,
    ) {
        if let Some(slice) = self.loaded_slices.take(reg) {
            trace_jit!("Loading register {reg} from slice {slice}");
            self.allocations
                .load_spill_directives
                .add_load_after(reg, slice);
        }
        self.read_regs.remove(reg);
        self.written_regs.remove(reg);
    }

    /// Spill the register if it is the last write occurrence (viewing in forward direction)
    #[inline]
    fn spill_reg(
        &mut self,
        reg: HostReg,
        slice: JIRSlice,
    ) {
        // If this is a write operand, we need to make sure that it is spilled correctly on the first write we
        // encounter (viewing in reverse).

        if !self.written_regs.contains(reg) {
            // The register was not written to the context yet. Insert a write now.
            self.allocations
                .load_spill_directives
                .add_spill_after(reg, slice);
        }
    }
}
