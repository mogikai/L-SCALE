use crate::spec::moir::scope::MOIRLocalId;
use serde::{
    Deserialize,
    Serialize,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Each fragment will be resolved into a concrete string and then inserted into
/// its corresponding final place. This happens recursively for subtables and
/// is dependent on the scope.
pub(crate) enum MOIRDisplayFragment {
    Literal(String),
    Local(MOIRLocalId),
}

/// The display section for converting a lifted byte sequence into a more human-readable disassembly
/// string.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct MOIRDisplay {
    pub(crate) fragments: Vec<MOIRDisplayFragment>,
}

impl MOIRDisplay {
    pub(crate) fn new(fragments: Vec<MOIRDisplayFragment>) -> Self {
        Self { fragments }
    }
}
