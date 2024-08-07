use crate::new_symbol_id_type;
use crate::spec::moir::code::MOIRInstruction;
use crate::spec::moir::varnode::MOIRVarNodeId;

#[derive(Debug, Default)]
pub struct MOIRMacro {
    pub arguments: Vec<MOIRVarNodeId>,
    pub code: Vec<MOIRInstruction>,
}

new_symbol_id_type! { pub struct MOIRMacroId; }
