mod instruction;
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
use strum::EnumCount;

#[derive(Debug)]
pub struct RegAllocHostConstraints {
    usable_registers: HostRegSet,
    /// Some instructions need a scratch register. This register can be used
    /// by any instruction and any directive, but it gives no guarantees
    /// to its contents before or after the corresponding instruction or directive.
    /// This register can never be allocated to a slice.
    /// Each class has one scratch register.
    scratch_register: [HostReg; RegClass::COUNT],
}

impl RegAllocHostConstraints {
    pub const fn new(usable_registers: HostRegSet) -> Self {
        RegAllocHostConstraints {
            usable_registers,
            scratch_register: [HostReg::invalid(); RegClass::COUNT],
        }
    }

    pub const fn with_scratch(
        mut self,
        reg: HostReg,
    ) -> Self {
        self.usable_registers.remove(reg);
        self.scratch_register[reg.class() as usize] = reg;
        self
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
    // written_regs: HostRegSet,
    /// Keeps track of registers that have been read previously.
    // read_regs: HostRegSet,

    /// TODO: Add the appropriate env constraints. We probably need to be able to exclude some registers for example.
    ///       It might also make sense to define the set of usable registers this way.
    host_constraints: RegAllocHostConstraints,

    /// Keeps track of the available registers for an instruction.
    /// This set of registers is reset for each instruction according to
    /// the instructions' and environments' constraints.
    available_registers: HostRegSet,
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
        let available_registers = host_constraints.usable_registers;
        RegisterAllocator::<'a, E> {
            target_block: translation_unit,
            emitter_requirements,
            allocations: AllocatorResult::new(
                translation_unit,
                emitter_requirements,
                host_constraints.scratch_register,
            ),
            loaded_slices: Default::default(),
            host_constraints,
            available_registers,
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
            self.allocations
                .load_spill_directives
                .add_load_before(reg, slice);
        }
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
        self.available_registers = self.host_constraints.usable_registers;
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

        for (op_id, operand) in operands.fixed() {
            let allocation = self.alloc_operand(operand)?;
            trace_jit!("Allocated fixed reg operand {operand:?} for instruction {inst} as {allocation:?}");
            self.allocations.set_allocation(inst, op_id, allocation);
        }

        for (op_id, operand) in operands.non_fixed() {
            let allocation = self.alloc_operand(operand)?;
            trace_jit!("Allocated operand {operand:?} for instruction {inst} as {allocation:?}");
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
        for (reg, _) in self.loaded_slices.iter_class(class) {
            available_regs.remove(reg);
        }
        available_regs
            .into_iter()
            .filter(|r| r.class() == class)
            .nth(0)
            .ok_or_else(|| -> JitError { todo!("Error handling") })
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
            self.spill_reg(reg);
            temp_regs.add(reg);
        }

        for reg in self
            .available_registers
            .iter_class(RegClass::Float)
            .take(request.requested_float() as usize)
        {
            trace_jit!("Allocating temp float register {reg}");
            self.available_registers.remove(reg);
            self.spill_reg(reg);
            temp_regs.add(reg);
        }

        for reg in self
            .available_registers
            .iter_class(RegClass::Vector)
            .take(request.requested_vector() as usize)
        {
            trace_jit!("Allocating temp vector register {reg}");
            self.available_registers.remove(reg);
            self.spill_reg(reg);
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
        self.spill_reg(reg);
        self.loaded_slices.insert(reg, operand.slice);
        Ok(Allocation::Register(reg))
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
        if existing_slice == operand.slice {
            if let OperandConstraint::FixedReg(fixed_reg) = operand.constraint {
                // If the fixed constraint specifies a different register, we have to move the allocation there
                if reg != fixed_reg {
                    trace_jit!("Allocation is wanted in {fixed_reg}, but is already in {reg}. Moving allocation");
                    // We add a move such that the allocation will be in `reg` after the instruction.
                    // This is necessary, since any following instructions (in forward order)
                    // are already allocated to the register `reg`.
                    self.allocations
                        .load_spill_directives
                        .add_move_after(reg, fixed_reg, existing_slice.size);
                    // And update the loaded slices to point to the new reg
                    // We update this for the reverse direction, since the loaded_slices map is relevant for not yet
                    // allocated slices.
                    self.loaded_slices.r#move(fixed_reg, reg);
                    // We remove the fixed_reg, since this is now not available anymore for this instruction.
                    self.available_registers.remove(fixed_reg);
                    return Ok(fixed_reg);
                }
            }
            self.available_registers.remove(reg);
            Ok(reg)
        } else {
            if !self.available_registers.contains(reg) && existing_slice.offset != operand.slice.offset {
                // We already allocated something to `reg` and it has a different offset.
                todo!("Different offset aliasing in the same instruction currently unsupported")
            }
            // After the instruction is finished, adjust the slice to the one that was previously expected.
            // (that is expected next when viewing in forward direction)
            self.allocations
                .load_spill_directives
                .add_adjust_after(reg, operand.slice, existing_slice);
            self.available_registers.remove(reg);
            Ok(reg)
        }
    }

    fn alloc_operand(
        &mut self,
        operand: &OperandRequest,
    ) -> JITResult<Allocation> {
        let allocation = if let Some((reg, existing_slice)) = self.get_loaded_slice_group(operand) {
            Allocation::Register(self.ensure_valid_reg_allocation(reg, existing_slice, operand)?)
        } else {
            // Slice group is not in any register. We can allocate one from the free regs.
            self.alloc_unloaded_operand(operand)?
        };

        if let Allocation::Register(reg) = allocation
            && let OperandKind::Write = operand.kind
        {
            // If this is a write operand, we need to make sure that it is spilled correctly on the first write we
            // encounter (viewing in reverse).
            // TODO: Only spill on first write (viewing in reverse)
            self.allocations
                .load_spill_directives
                .add_spill_after(reg, operand.slice);
        }
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
        self.spill_reg(clobber);
        self.available_registers.remove(clobber);
    }

    /// Spill the provided register at the `after` position of the current instruction.
    #[inline]
    fn spill_reg(
        &mut self,
        reg: HostReg,
    ) {
        // Spilling the register
        if let Some(slice) = self.loaded_slices.take(reg) {
            trace_jit!("Spilling register {reg} to slice {slice}");
            self.allocations
                .load_spill_directives
                .add_load_after(reg, slice);
        }
    }
}
