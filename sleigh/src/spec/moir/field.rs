use crate::new_symbol_id_type;
use crate::spec::builder::StringId;
use crate::spec::moir::token::MOIRTokenId;
use std::ops::Range;

use super::context::MOIRContextId;
use super::varnode::MOIRVarNodeId;

// context and token fields are treated the same due to their similarity and are
// only discernible by their respective parent (either a token or a context definition)
#[derive(Debug)]
pub enum MOIRFieldParent {
    Token(MOIRTokenId),
    Context(MOIRContextId),
}

#[derive(Debug)]
pub struct MOIRField {
    /// The token or context this field is associated with.
    pub parent: MOIRFieldParent,
    pub bits: Range<u64>,
    /// Attach variables can attach varnodes to the field.
    /// Each value is then mapped to a specific varnode identifier.
    /// e.g. 0 -> EAX, 1 -> EBX, 2 -> ECX, 3 -> EDX
    pub variables: Vec<MOIRVarNodeId>,
    /// Attach values can attach values to the field.
    /// Each value is then mapped to another specific value.
    /// e.g. 0 -> 1, 1 -> 2, 2 -> 4, 3 -> 8
    pub values: Vec<u64>,
    /// Attach different strings to the field to change the display characteristic. This has no
    /// influence on the semantic meaning.
    pub strings: Vec<StringId>,
}

new_symbol_id_type! { pub struct MOIRFieldId; }
