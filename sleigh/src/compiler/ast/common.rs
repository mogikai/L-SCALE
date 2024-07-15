use crate::compiler::StringId;

#[derive(Debug)]
pub struct SASTIdent(pub StringId);
#[derive(Debug)]
pub struct SASTString(pub StringId);
