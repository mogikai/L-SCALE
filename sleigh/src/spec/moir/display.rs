use serde::{Deserialize, Serialize};

use crate::spec::FieldId;

use super::{scope::MOIRTemporaryId, table::TableId, varnode::{MOIRReference, VarNodeId}};

#[derive(Debug, Serialize, Deserialize)]
/// Each fragment will be resolved into a concrete string and then inserted into
/// its corresponding final place. This happens recursively for subtables and
/// is dependent on the scope.
pub(crate) enum MOIRDisplayFragment {
    Literal(String),
    Local(MOIRReference),
}

/// The display section for converting a lifted byte sequence into a more human-readable disassembly
/// string.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct MOIRDisplay {
    pub(crate) fragments: Vec<MOIRDisplayFragment>
}

impl MOIRDisplay {
    pub(crate) fn new(fragments: Vec<MOIRDisplayFragment>) -> Self {
        Self { fragments }
    }
}
