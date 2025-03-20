use mogi_utilities::new_map_key;

use crate::new_typed_index_table_id;

use super::varnode::VarNodeId;

#[derive(Debug)]
pub struct MOIRTBitrangeRegister {
    pub source_varnode: VarNodeId,
    pub offset: u64, // offset into source varnode
    pub size: u64,   // number of bits
}

new_map_key! { pub struct BitrangeRegisterId; "BR"; }
