use mogi_utilities::new_map_key;

use crate::new_typed_index_table_id;
use crate::spec::moir::endianness::MOIREndianness;

use super::common::MOIRSize;

#[derive(Debug)]
pub struct MOIRToken {
    /// The number of bytes the decoder shall decode for this token.
    pub bytes: MOIRSize,
    pub endianness_override: Option<MOIREndianness>,
}

new_map_key! { pub struct TokenId;  "TOK"; }
