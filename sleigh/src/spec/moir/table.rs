use crate::new_typed_index_table_id;
use crate::spec::moir::address_space::SpaceId;
use crate::spec::moir::constructor::ConstructorId;
use crate::spec::moir::scope::{
    MOIRMaybeSize,
    MOIRScope,
};
use crate::spec::moir::varnode::MOIRLocal;

#[derive(Debug, Clone, Default)]
pub struct MOIRTable {
    // TODO: how to actually implement this?
    // Ideally, each constructor inserts its bit pattern into a jump table
    // The first pattern in the op points to the second pattern and so on until
    // we reach the disassembly actions matchers. This can be a list, if multiple constructors
    // match. We then have to match each of these sections, and use the one with the most
    // constraints to finally lift the instruction.
    bit_matching_table: Vec<Vec<()>>,
    pub constructors: Vec<ConstructorId>,
}

// TODO: This probably needs to look a little different than it does atm
#[derive(Debug, Copy, Clone)]
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
