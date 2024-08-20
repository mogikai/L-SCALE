use crate::new_typed_index_table_id;

use super::varnode::MOIRVarNodeId;

#[derive(Debug)]
pub struct MOIRTBitrangeRegister {
    pub source_varnode: MOIRVarNodeId,
    pub offset: u64, // offset into source varnode
    pub size: u64,   // number of bits
}

new_typed_index_table_id! { pub struct MOIRBitrangeRegisterId; }
