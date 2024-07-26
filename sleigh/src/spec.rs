pub mod address_space;
mod moir;
pub mod varnode;

use crate::compiler::StringId;
use crate::spec::address_space::MOIRAddressSpace;
use crate::spec::moir::MOIRMacro;
use crate::spec::varnode::MOIRVarNode;
use slotmap::{
    new_key_type,
    SecondaryMap,
    SlotMap,
};
use std::collections::HashMap;
use string_interner::backend::StringBackend;
use string_interner::StringInterner;

new_key_type! { pub struct MOIRAddressSpaceId; }
new_key_type! { pub struct MOIRVarNodeId; }
new_key_type! { pub struct MOIRIdentId; }

#[derive(Debug, Default)]
pub struct ProcessorSpecification {
    pub(super) interner: StringInterner<StringBackend>,
    pub(super) identifiers: SlotMap<MOIRIdentId, StringId>,
    pub(super) mir_macros: SecondaryMap<MOIRIdentId, MOIRMacro>,
    pub(super) address_spaces: SlotMap<MOIRAddressSpaceId, MOIRAddressSpace>,
    pub(super) varnodes: SlotMap<MOIRVarNodeId, MOIRVarNode>,

    // lookup tables:
    pub(super) string_to_ident_mapping: HashMap<StringId, MOIRIdentId>,
}

impl ProcessorSpecification {
    pub(crate) fn resolve_ident_to_id<I>(
        &mut self,
        ident: I,
    ) where
        I: Into<StringId>,
    {
        let ident = ident.into();
        self.string_to_ident_mapping
            .entry(ident)
            .or_insert_with(|| self.identifiers.insert(ident));
    }
}

impl ProcessorSpecification {
    pub fn new() -> Self {
        Self {
            interner: Default::default(),
            identifiers: Default::default(),
            mir_macros: Default::default(),
            address_spaces: Default::default(),
            varnodes: Default::default(),
            string_to_ident_mapping: Default::default(),
        }
    }
}
