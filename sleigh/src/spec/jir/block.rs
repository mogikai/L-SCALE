use crate::spec::jir::jcode::JIRInstruction;

#[derive(Debug)]
pub struct JIRBlock {
    instructions: Vec<JIRInstruction>,
}
