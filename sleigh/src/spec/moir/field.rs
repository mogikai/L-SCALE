use super::common::MOIRSize;
use super::context::ContextId;
use super::varnode::VarNodeId;
use crate::spec::moir::token::TokenId;
use mogi_sleigh_parser::StringId;
use mogi_utilities::new_map_key;
use serde_derive::{
    Deserialize,
    Serialize,
};

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct MOIRBitRange {
    pub start: MOIRSize,
    pub end: MOIRSize,
}

impl MOIRBitRange {
    pub fn new(
        start: MOIRSize,
        end: MOIRSize,
    ) -> Self {
        Self { start, end }
    }

    pub fn bits(&self) -> MOIRSize {
        MOIRSize::try_from(self.end - self.start).unwrap()
    }

    pub fn bytes(&self) -> Option<MOIRSize> {
        let bits = self.bits();
        if bits % 8 == 0 { Some(bits / 8) } else { None }
    }

    /// Generate a mask for the bit range without any shifts.
    pub fn mask(&self) -> u64 {
        if self.bits() as u32 == u64::BITS {
            // prevent overflow
            u64::MAX
        } else {
            (1 << self.bits()) - 1
        }
    }

    /// Generate a mask for the bit range including shifts.
    pub fn shifted_mask(&self) -> u64 {
        self.mask() << self.start
    }

    /// Right-shift `v` by `self.start` and apply `self.mask()`
    pub fn extract(
        &self,
        v: u64,
    ) -> u64 {
        (v >> self.start) & self.mask()
    }

    /// Apply `self.mask()` to `v` and left-shift by `self.start`
    pub fn insert(
        &self,
        v: u64,
    ) -> u64 {
        (v & self.mask()) << self.start
    }
}

impl std::ops::BitAnd for MOIRBitRange {
    type Output = Self;

    /// Calculates the intersection of `self` and `rhs`. Note that this operation keeps ranges
    /// continuous so it's well-defined.
    fn bitand(
        self,
        rhs: Self,
    ) -> Self::Output {
        MOIRBitRange::new(self.start.max(rhs.start), self.end.min(rhs.end))
    }
}

// context and token fields are treated the same due to their similarity and are
// only discernible by their respective parent (either a token or a context definition)
#[derive(Debug, Hash, PartialOrd, Ord, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MOIRFieldParent {
    Token(TokenId),
    Context(ContextId),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

new_map_key! { pub struct FieldId; "F"; }
