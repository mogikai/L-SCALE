use crate::new_symbol_id_type;
use crate::spec::endianness::MOIREndianness;

#[derive(Debug)]
pub struct MOIRToken {
    /// The number of bits the decoder shall decode for this token.
    pub bits: u64,
    pub endianness_override: Option<MOIREndianness>,
}

new_symbol_id_type! { pub struct MOIRTokenId; }
