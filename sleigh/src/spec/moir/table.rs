use std::collections::BTreeSet;

use crate::compiler::build::constructor::matcher::{Matcher, MatcherId};
use crate::new_typed_index_table_id;
use crate::spec::moir::address_space::SpaceId;
use crate::spec::moir::common::MOIRSize;
use crate::spec::moir::constructor::ConstructorId;
use crate::spec::moir::scope::{
    MOIRMaybeSize,
    MOIRScope,
};
use crate::spec::moir::varnode::{
    MOIRConst,
    MOIRLocal,
    MOIRSlice,
};
use crate::spec::typed_index_table::TypedIndexTable;

#[derive(Debug, Default)]
pub struct MOIRTable {
    pub(crate) matchers: TypedIndexTable<MatcherId, Matcher>,
    pub constructors: Vec<ConstructorId>,
}

// TODO: This probably needs to look a little different than it does atm
#[derive(Debug)]
pub enum MOIRExport {
    /// The location exported is the local varnode
    Local(MOIRLocal),
    /// The location exported is the memory location referred to by the varnode.
    /// Any accesses to a reference are translated into a load or store operation.
    MemReference {
        address: MOIRLocal,
        space: SpaceId,
        size: MOIRMaybeSize,
    },
}

impl MOIRExport {
    pub fn size(
        &self,
        scope: &MOIRScope,
    ) -> MOIRMaybeSize {
        match self {
            MOIRExport::Local(local) => local.size(scope),
            MOIRExport::MemReference { size, .. } => *size,
        }
    }
}

new_typed_index_table_id! { pub struct TableId; "TBL"; }
