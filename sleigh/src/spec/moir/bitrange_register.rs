use crate::new_symbol_id_type;

use super::varnode::MOIRVarNodeId;

#[derive(Debug)]
pub struct MOIRTBitrangeRegister {
    pub source_varnode: MOIRVarNodeId,
    pub offset: u64, // offset into source varnode
    pub size: u64,   // number of bits
}

new_symbol_id_type! { pub struct MOIRBitrangeRegisterId; }
