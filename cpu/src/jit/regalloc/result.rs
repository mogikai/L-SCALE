use crate::jit::api::JITResult;
use crate::jit::host::register::HostReg;
use crate::jit::trace_jit;
use mogi_sleigh::spec::jir::basic_block::InstId;
use mogi_sleigh::spec::jir::values::JIRSlice;
use std::ops::Range;

#[derive(Debug)]
pub enum Allocation {
    /// The operand is loaded in the provided host register.
    Register(HostReg),
    /// The operand lives in memory in the provided slice and must
    /// be used as a memory operand. The allocator can be instructed
    /// to not use memory operands. However, for seldom used operands,
    /// it may make sense to directly use them from memory.
    Slice(JIRSlice),
}

#[derive(Debug)]
pub struct AllocatorResult {
    pub(super) allocations: Vec<Allocation>,
    pub(super) allocation_ranges: Vec<Range<usize>>,
    pub(super) load_spill_directives: LoadSpillDirectives,
}

impl AllocatorResult {
    pub fn new(inst_count: usize) -> Self {
        Self {
            allocations: vec![],
            allocation_ranges: vec![0..0; inst_count],
            load_spill_directives: LoadSpillDirectives::new(inst_count),
        }
    }
}

#[derive(Debug)]
pub enum LoadSpillDirective {
    Load(HostReg, JIRSlice),
    Spill(HostReg, JIRSlice),
    Move(HostReg, HostReg),
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

    pub fn add_move_before(
        &mut self,
        dst: HostReg,
        src: HostReg,
    ) {
        self.before_directives
            .push(LoadSpillDirective::Move(dst, src));
    }

    pub fn add_move_after(
        &mut self,
        dst: HostReg,
        src: HostReg,
    ) {
        self.after_directives
            .push(LoadSpillDirective::Move(dst, src));
    }
}

#[test]
fn test() {
    // Rust does some magic, such that the total size of allocation is still 8 bytes (it fits into a 64 bit int/reg)
    assert!(size_of::<Allocation>() <= 8);
}
