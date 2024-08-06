use crate::new_symbol_id_type;
use crate::spec::token::MOIRTokenId;
use std::ops::Range;

#[derive(Debug)]
pub struct MOIRField {
    /// The token this field is associated with.
    pub token: MOIRTokenId,
    pub bits: Range<u64>,
    // TODO: attach variables can attach varnodes to the field.
    // each value is then mapped to a specific varnode identifier.
    // e.g. 0 -> EAX, 1 -> EBX, 2 -> ECX, 3 -> EDX
    pub variables: Vec<()>,
    pub values: Vec<()>,
    pub strings: Vec<()>,
}

new_symbol_id_type! { pub struct MOIRFieldId; }
