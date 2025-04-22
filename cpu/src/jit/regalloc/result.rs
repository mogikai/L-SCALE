use crate::jit::api::JITResult;
use crate::jit::host::register::{
    HostReg,
    HostRegSet,
    RegClass,
};
use crate::jit::regalloc::requirements::EmitterAllocRequirements;
use crate::jit::trace_jit;
use mogi_sleigh::spec::jir::basic_block::{
    InstId,
    TargetBasicBlock,
};
use mogi_sleigh::spec::jir::common::JIRSize;
use mogi_sleigh::spec::jir::values::JIRSlice;
use std::ops::Range;
use strum::EnumCount;

#[derive(Debug, Copy, Clone)]
pub enum Allocation {
    /// The operand is loaded in the provided host register.
    Register(HostReg),
    /// The operand lives in memory in the provided slice and must
    /// be used as a memory operand. The allocator can be instructed
    /// to not use memory operands. However, for seldom used operands,
    /// it may make sense to directly use them from memory.
    Slice(JIRSlice),
    /// No allocation exists for this operand.
    None,
}

#[derive(Debug)]
pub struct AllocatorResult {
    pub(super) allocations: Vec<Allocation>,
    pub(super) allocation_offsets: Vec<usize>,
    pub(super) temp_allocations: Vec<HostRegSet>,
    pub(super) clobbers: Vec<HostRegSet>,
    pub(super) load_spill_directives: LoadSpillDirectives,
    scratch_registers: [HostReg; RegClass::COUNT],
}

impl AllocatorResult {
    pub fn new<E>(
        target_block: &TargetBasicBlock,
        requirements: &E,
        scratch_registers: [HostReg; RegClass::COUNT],
    ) -> Self
    where
        E: EmitterAllocRequirements,
    {
        let mut allocation_offsets = Vec::new();
        // we only need exactly this amount
        allocation_offsets.reserve_exact(target_block.instruction_count());
        let mut operands_so_far = 0;

        // We want to save an offset in the allocations vector for each instruction, and reserve the next
        // elements to fit the allocations for the operands of the instruction.
        for inst in target_block.inst_ids() {
            let operands = requirements.operands(inst).len();
            allocation_offsets.push(operands_so_far);
            operands_so_far += operands;
        }

        let allocations = vec![Allocation::None; operands_so_far];

        Self {
            // Most operations have one output and two inputs, so we slightly overestimate here.
            allocations,
            allocation_offsets,
            // By default, each instruction gets an empty set of temporaries
            temp_allocations: vec![HostRegSet::empty(); target_block.instruction_count()],
            clobbers: vec![HostRegSet::empty(); target_block.instruction_count()],
            load_spill_directives: LoadSpillDirectives::new(target_block.instruction_count()),
            scratch_registers,
        }
    }

    /// Set the allocation for operand `op_id` of instruction `inst` to `allocation`.
    pub(super) fn set_allocation(
        &mut self,
        inst: InstId,
        op_id: usize,
        allocation: Allocation,
    ) {
        let alloc_id = self.allocation_offsets[inst] + op_id;
        self.allocations[alloc_id] = allocation;
    }

    pub fn scratch_register(
        &self,
        class: RegClass,
    ) -> HostReg {
        self.scratch_registers[class as usize]
    }

    #[inline(always)]
    pub fn get_allocation(
        &self,
        inst: InstId,
        op_id: usize,
    ) -> Allocation {
        let alloc_id = self.allocation_offsets[inst] + op_id;
        debug_assert!(
            alloc_id
                < self
                    .allocation_offsets
                    .get(inst + 1)
                    .copied()
                    .unwrap_or(self.allocation_offsets.len())
        );
        self.allocations[alloc_id]
    }

    pub fn get_temp_allocations(
        &self,
        inst: InstId,
    ) -> HostRegSet {
        self.temp_allocations[inst]
    }

    pub fn get_clobbers(
        &self,
        inst: InstId,
    ) -> HostRegSet {
        self.clobbers[inst]
    }

    pub fn directives(&self) -> &LoadSpillDirectives {
        &self.load_spill_directives
    }
}

#[derive(Debug)]
pub enum LoadSpillDirective {
    Load(HostReg, JIRSlice),
    Spill(HostReg, JIRSlice),
    /// Adjust the aliased slice in the register to the new slice.
    /// This spills any modifications of old_slice that are not in new_slice
    /// and loads any new content from new_slice that is not in old_slice.
    /// This is only allowed for slices with equal offsets for now.
    Adjust {
        reg: HostReg,
        old_slice: JIRSlice,
        new_slice: JIRSlice,
    },
    /// Move the register `src` to the register `dst`since the allocation of a slice moved.
    /// `size` specifies the size of the move.
    Move {
        dst: HostReg,
        src: HostReg,
        size: JIRSize,
    },
}

#[derive(Debug, Default, Clone)]
struct InstToDirectiveMapping {
    before: Range<usize>,
    after: Range<usize>,
}

impl InstToDirectiveMapping {
    pub fn is_empty(&self) -> bool {
        self.before.start == self.before.end && self.after.start == self.after.end
    }
}

/// This struct keeps track of the load or spill instructions required per instruction.
/// These load or spill directives are created by the register allocator and need to be inserted by the
/// jit compiler before and after each instruction.
#[derive(Debug)]
pub struct LoadSpillDirectives {
    before_directives: Vec<LoadSpillDirective>,
    after_directives: Vec<LoadSpillDirective>,
    /// Maps each instruction id to the range of `before` directives and `after` directives.
    inst_to_directives: Vec<InstToDirectiveMapping>,
    current_inst: InstId,
}

impl LoadSpillDirectives {
    pub fn new(inst_count: usize) -> Self {
        Self {
            before_directives: vec![],
            after_directives: vec![],
            inst_to_directives: vec![InstToDirectiveMapping::default(); inst_count],
            current_inst: 0,
        }
    }

    pub fn before_inst_directives(
        &self,
        inst: InstId,
    ) -> impl Iterator<Item = &LoadSpillDirective> {
        self.before_directives[self.inst_to_directives[inst].before.clone()]
            .iter()
            .rev()
    }

    pub fn after_inst_directives(
        &self,
        inst: InstId,
    ) -> impl Iterator<Item = &LoadSpillDirective> {
        self.after_directives[self.inst_to_directives[inst].after.clone()]
            .iter()
            .rev()
    }
}

impl LoadSpillDirectives {
    pub fn start_inst_directives(
        &mut self,
        inst: InstId,
    ) -> JITResult {
        if !self.inst_to_directives[inst].is_empty() {
            todo!("Emit error: Cannot start adding inst directives to non empty directive.")
        }

        let inst_to_directives = &mut self.inst_to_directives[self.current_inst];
        inst_to_directives.before.end = self.before_directives.len();
        inst_to_directives.after.end = self.after_directives.len();
        if !inst_to_directives.before.is_empty() || !inst_to_directives.after.is_empty() {
            trace_jit!(
                "Added {} before and {} after directives for inst {}:{:?}{:?}",
                inst_to_directives.before.len(),
                inst_to_directives.after.len(),
                self.current_inst,
                &self.before_directives[inst_to_directives.before.clone()],
                &self.after_directives[inst_to_directives.after.clone()],
            );
        }

        self.current_inst = inst;
        // Set the indices to the end of the current vectors.
        self.inst_to_directives[inst].after = self.after_directives.len()..self.after_directives.len();
        self.inst_to_directives[inst].before = self.before_directives.len()..self.before_directives.len();
        Ok(())
    }

    /// Update the directives range for the current instruction. This is mainly used
    /// at the end of a block, where some additional directives need to be inserted
    /// to make sure that everything is loaded properly.
    pub fn update_current_inst_directives_end(&mut self) {
        let inst_to_directives = &mut self.inst_to_directives[self.current_inst];
        inst_to_directives.before.end = self.before_directives.len();
        inst_to_directives.after.end = self.after_directives.len();
    }

    pub fn add_load_before(
        &mut self,
        reg: HostReg,
        slice: JIRSlice,
    ) {
        trace_jit!("Adding load before inst {}: Loading {:?} into {:?}", self.current_inst, slice, reg);
        self.before_directives
            .push(LoadSpillDirective::Load(reg, slice));
    }

    pub fn add_load_after(
        &mut self,
        reg: HostReg,
        slice: JIRSlice,
    ) {
        trace_jit!("Adding load after inst {}: Loading {:?} into {:?}", self.current_inst, slice, reg);
        self.after_directives
            .push(LoadSpillDirective::Load(reg, slice));
    }

    pub fn add_spill_before(
        &mut self,
        reg: HostReg,
        slice: JIRSlice,
    ) {
        trace_jit!("Adding spill before inst {}: Spilling {:?} into {:?}", self.current_inst, reg, slice);
        self.before_directives
            .push(LoadSpillDirective::Spill(reg, slice));
    }

    pub fn add_spill_after(
        &mut self,
        reg: HostReg,
        slice: JIRSlice,
    ) {
        trace_jit!("Adding spill after inst {}: Spilling {:?} into {:?}", self.current_inst, reg, slice);
        self.after_directives
            .push(LoadSpillDirective::Spill(reg, slice));
    }

    pub fn add_adjust_before(
        &mut self,
        reg: HostReg,
        old_slice: JIRSlice,
        new_slice: JIRSlice,
    ) {
        self.before_directives.push(LoadSpillDirective::Adjust {
            reg,
            old_slice,
            new_slice,
        });
    }

    pub fn add_adjust_after(
        &mut self,
        reg: HostReg,
        old_slice: JIRSlice,
        new_slice: JIRSlice,
    ) {
        self.after_directives.push(LoadSpillDirective::Adjust {
            reg,
            old_slice,
            new_slice,
        })
    }

    pub fn add_move_before(
        &mut self,
        dst: HostReg,
        src: HostReg,
        size: JIRSize,
    ) {
        self.before_directives
            .push(LoadSpillDirective::Move { dst, src, size });
    }

    pub fn add_move_after(
        &mut self,
        dst: HostReg,
        src: HostReg,
        size: JIRSize,
    ) {
        self.after_directives
            .push(LoadSpillDirective::Move { dst, src, size });
    }
}

#[test]
fn test() {
    // Rust does some magic, such that the total size of allocation is still 8 bytes (it fits into a 64 bit int/reg)
    assert!(size_of::<Allocation>() <= 8);
}
