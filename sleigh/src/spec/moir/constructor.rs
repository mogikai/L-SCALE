use crate::spec::moir::code::MOIRInstruction;
use crate::spec::moir::scope::MOIRScope;

#[derive(Debug)]
pub struct MOIRConstructor {
    pub local_scope: MOIRScope,
    pub instructions: Vec<MOIRInstruction>,
}
