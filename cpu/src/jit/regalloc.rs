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
};
use crate::jit::regalloc::requirements::{
    OperandConstraint,
    OperandKind,
    OperandPos,
    OperandRequest,
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

#[derive(Debug)]
pub struct RegAllocHostConstraints {
    pub usable_registers: HostRegSet,
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
            allocations: AllocatorResult::new(translation_unit.instruction_count()),
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
        for inst in self.target_block.jir_block_inst_ids(block).unwrap().rev() {
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
        if clobbers.is_empty() && operands.all().len() == 0 {
            trace_jit!("No operands or clobbers for {inst}, skipping allocation");
            return Ok(());
        }
        trace_jit!("");
        trace_jit!("Allocating registers for {inst}. It is clobbering {clobbers} and requesting {operands:?}");
        trace_jit!("Available registers at the start:\n{}", self.available_registers,);

        for operand in operands.fixed() {
            self.reserve_reg_for_fixed_operand(operand)?;
        }
        for clobber in clobbers {
            self.clear_clobber(clobber);
        }

        trace_jit!("Available registers after fixed reg and clobber constraints:\n{}", self.available_registers,);

        for operand in operands.write() {
            self.alloc_write_operand(operand)?;
        }

        for operand in operands.read() {
            self.alloc_read_operand(operand)?;
        }

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

    /// Checks if the slice group is already loaded, and returns the host reg and loaded slice
    /// if it exists.
    fn get_loaded_slice_group(
        &self,
        operand: &OperandRequest,
    ) -> Option<(HostReg, JIRSlice)> {
        self.loaded_slices
            .iter_class(operand.class)
            .find_map(|(reg, loaded_slice)| {
                if loaded_slice.group_id == operand.slice.group_id {
                    Some((reg, *loaded_slice))
                } else {
                    None
                }
            })
    }

    /// Allocate a register for the provided operand.
    /// Returns an error, if no operand could be allocated.
    fn alloc_register(
        &mut self,
        operand: &OperandRequest,
    ) -> JITResult<HostReg> {
        let reg = self
            .available_registers
            .into_iter()
            .filter(|r| r.class() == operand.class)
            .nth(0)
            .ok_or_else(|| -> JitError { todo!("Error handling") })?;

        self.available_registers.remove(reg);

        /// Spill the register if it is occupied by a slice.
        self.spill_reg(reg);
        Ok(reg)
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
        trace_jit!("Slice group is already loaded in register {reg} with slice {existing_slice:?}.");
        if existing_slice.offset != operand.slice.offset || existing_slice.size != operand.slice.size {
            todo!("Aliased loads currently not supported")
        }
        if let OperandConstraint::FixedReg(fixed_reg) = operand.constraint {
            if reg != fixed_reg {
                todo!("Move allocation to target_reg")
            }
        }
        self.available_registers.remove(reg);
        Ok(reg)
    }

    fn alloc_write_operand(
        &mut self,
        operand: &OperandRequest,
    ) -> JITResult<Allocation> {
        let OperandKind::Write = operand.kind else {
            panic!("Logic error: Write operand must have kind Write.")
        };
        trace_jit!("Allocating write operand {operand:?}");
        if let Some((reg, existing_slice)) = self.get_loaded_slice_group(operand) {
            Ok(Allocation::Register(self.ensure_valid_reg_allocation(reg, existing_slice, operand)?))
        } else {
            // Slice group is not in any register. We can allocate one from the free regs.

            let reg = self.alloc_register(operand)?;
            // We are allocating a write operand which needs to be preserved.
            // This needs to be done for the first occurrence (when viewing in reverse),
            // since this will be the last time this value is written.
            // TODO: Only spill if this is the first occurrence
            self.allocations
                .load_spill_directives
                .add_spill_after(reg, operand.slice);
            self.loaded_slices.insert(reg, operand.slice);
            Ok(Allocation::Register(reg))
        }
    }

    //noinspection DuplicatedCode
    fn alloc_read_operand(
        &mut self,
        operand: &OperandRequest,
    ) -> JITResult<Allocation> {
        let OperandKind::Read = operand.kind else {
            panic!("Logic error: Write operand must have kind Write.")
        };
        trace_jit!("Allocating read operand {operand:?}");
        if let Some((reg, existing_slice)) = self.get_loaded_slice_group(operand) {
            Ok(Allocation::Register(self.ensure_valid_reg_allocation(reg, existing_slice, operand)?))
        } else {
            // Slice group is not in any register. We can allocate one from the free regs.
            let reg = self.alloc_register(operand)?;
            self.loaded_slices.insert(reg, operand.slice);
            Ok(Allocation::Register(reg))
        }
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

        // TODO: Is this the correct spill location?
        self.spill_reg(target_reg);
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
