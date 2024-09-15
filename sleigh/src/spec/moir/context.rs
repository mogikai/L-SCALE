use crate::new_typed_index_table_id;

use super::varnode::VarNodeId;

#[derive(Debug, Default)]
pub struct MOIRContextFieldId(usize);

impl From<usize> for MOIRContextFieldId {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

#[derive(Debug)]
pub struct MOIRContext {
    /// The varnode the context is defined on top of.
    pub varnode: VarNodeId,
}

// define MOIRContextId as a key for a SlotMap instead because it's not a symbol
new_typed_index_table_id! { pub struct MOIRContextId; "CNTXT"; }
