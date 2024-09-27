use slotmap::new_key_type;
use std::ops::Range;

#[derive(Debug, Default)]
pub struct JIRBlock {
    /// The range in the instruction stream where this blocks instructions are located.
    pub range: Range<usize>,
    /// The id of the block that follows this one.
    pub successor: Option<JIRBlockId>,
}

new_key_type! { pub struct JIRBlockId; }
