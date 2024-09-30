use mogi_utilities::new_map_key;
use slotmap::new_key_type;
use std::ops::Range;

#[derive(Debug, Default, Clone)]
pub struct JIRBlock {
    /// The range in the instruction stream where this blocks instructions are located.
    pub range: Range<usize>,
    /// The id of the block that follows this one.
    pub successor: Option<JIRBlockId>,
}

new_map_key! { pub struct JIRBlockId; "JIRB"; }
