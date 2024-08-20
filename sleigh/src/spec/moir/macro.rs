use crate::new_typed_index_table_id;
use crate::spec::moir::code::MOIRInstruction;
use crate::spec::moir::varnode::MOIRVarNodeId;

#[derive(Debug, Default)]
pub struct MOIRMacro {
    pub arguments: Vec<MOIRVarNodeId>,
    pub code: Vec<MOIRInstruction>,
}

new_typed_index_table_id! { pub struct MOIRMacroId; }
