use slotmap::new_key_type;

use super::varnode::MOIRVarNodeId;

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
    pub varnode: MOIRVarNodeId,
}

// define MOIRContextId as a key for a SlotMap instead because it's not a symbol
new_key_type! { pub struct MOIRContextId; }
