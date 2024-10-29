use crate::new_typed_index_table_id;
use crate::spec::builder::StringId;
use crate::spec::moir::token::TokenId;
use std::ops::Range;

use super::context::MOIRContextId;
use super::varnode::VarNodeId;

#[derive(Debug, Clone)]
pub struct MOIRBitRange {
    pub start: u8,
    pub end: u8,
}

impl MOIRBitRange {
    pub fn new(
        start: u8,
        end: u8,
    ) -> Self {
        Self { start, end }
    }

    pub fn size(&self) -> u8 {
        // a bit range is inclusive
        self.end - self.start + 1
    }

    /// Generate a mask for the bit range without any shifts.
    pub fn mask(&self) -> u64 {
        if self.size() as u32 == u64::BITS {
            // prevent overflow
            u64::MAX
        } else {
            (1 << self.size()) - 1
        }
    }

    /// Generate a mask for the bit range including shifts.
    pub fn mask_shift(&self) -> u64 {
        ((1 << self.size()) - 1) << self.start
    }

    pub fn extract_from(&self, v: u64) -> u64 {
        (v >> self.start) & self.mask()
    }
}

// context and token fields are treated the same due to their similarity and are
// only discernible by their respective parent (either a token or a context definition)
#[derive(Debug, Hash, Clone, Copy, PartialEq, Eq)]
pub enum MOIRFieldParent {
    Token(TokenId),
    Context(MOIRContextId),
}

#[derive(Debug)]
pub struct MOIRField {
    /// The token or context this field is associated with.
    pub parent: MOIRFieldParent,
    pub bits: MOIRBitRange,
    /// Attach variables can attach varnodes to the field.
    /// Each value is then mapped to a specific varnode identifier.
    /// e.g. 0 -> EAX, 1 -> EBX, 2 -> ECX, 3 -> EDX
    pub variables: Vec<VarNodeId>,
    /// Attach values can attach values to the field.
    /// Each value is then mapped to another specific value.
    /// e.g. 0 -> 1, 1 -> 2, 2 -> 4, 3 -> 8
    pub values: Vec<u64>,
    /// Attach different strings to the field to change the display characteristic. This has no
    /// influence on the semantic meaning.
    pub strings: Vec<StringId>,
}

new_typed_index_table_id! { pub struct FieldId; "F"; }
