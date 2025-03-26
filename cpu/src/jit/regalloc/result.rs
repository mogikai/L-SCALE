use crate::jit::host::register::HostReg;
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

#[derive(Debug, Default)]
pub struct AllocatorResult {
    allocations: Vec<Allocation>,
    allocation_ranges: Vec<Range<usize>>,
}

/// This struct keeps track of the load or spill instructions required per instruction.
/// These load or spill directives are created by the register allocator and need to be inserted by the
/// jit compiler before and after each instruction.
pub struct LoadSpillInstructions {}

#[test]
fn test() {
    // Rust does some magic, such that the total size of allocation is still 8 bytes (it fits into a 64 bit int/reg)
    assert_eq!(size_of::<Allocation>(), 8);
}
