use crate::new_typed_index_table_id;
use mogi_utilities::new_map_key;

use super::varnode::VarNodeId;

#[derive(Debug)]
pub struct MOIRContext {
    /// The varnode the context is defined on top of.
    pub varnode: VarNodeId,
}

// define MOIRContextId as a key for a SlotMap instead because it's not a symbol
new_map_key! { pub struct ContextId; "CNTXT"; }
