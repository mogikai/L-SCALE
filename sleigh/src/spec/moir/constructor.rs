use crate::new_typed_index_table_id;
use crate::spec::moir::code::MOIRInstruction;
use crate::spec::moir::scope::MOIRScope;

#[derive(Debug)]
pub struct MOIRConstructor {
    pub local_scope: MOIRScope,
    pub instructions: Vec<MOIRInstruction>,
}

new_typed_index_table_id! { pub struct ConstructorId; "C"; }
