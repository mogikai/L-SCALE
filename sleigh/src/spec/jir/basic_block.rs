use mogi_utilities::addresses::EVAddr;
use std::ops::Range;

#[derive(Debug)]
pub struct TargetBasicBlock {
    instructions: Range<usize>,
    /// A list of predecessor blocks that refer to this basic block.
    predecessors: Vec<EVAddr>,
    /// The successor of this basic block. Some blocks may not have any successor.
    /// This might be the case if the block issues some kind of halt instruction
    /// that stops execution.
    successor: Option<EVAddr>,
}
