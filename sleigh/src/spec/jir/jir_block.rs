use mogi_utilities::new_map_key;
use std::ops::Range;

#[derive(Debug, Default, Clone)]
pub struct JIRBlock {
    /// The range in the instruction stream where this blocks instructions are located.
    pub range: Range<usize>,
    /// The id of the block that follows this one.
    pub fallthrough: JIRBlockFallthrough,
}

new_map_key! { pub struct JIRBlockId; "JIRB"; }

#[derive(Debug, Default, Clone, Copy)]
pub enum JIRBlockFallthrough {
    #[default]
    /// A special marker for blocks that exit this instruction.
    /// This effectively means jump to the first block of the next instruction.
    NextInst,
    /// Fall through to the specified block
    Block(JIRBlockId),
}
