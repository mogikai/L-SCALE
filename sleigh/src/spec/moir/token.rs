use crate::new_typed_index_table_id;
use crate::spec::moir::endianness::MOIREndianness;

#[derive(Debug)]
pub struct MOIRToken {
    /// The number of bits the decoder shall decode for this token.
    pub bits: u64,
    pub endianness_override: Option<MOIREndianness>,
}

new_typed_index_table_id! { pub struct TokenId; }
