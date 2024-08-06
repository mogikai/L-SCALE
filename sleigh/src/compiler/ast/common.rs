use crate::compiler::ast::Spanned;
use crate::spec::StringId;

#[derive(Debug, Copy, Clone)]
pub struct SASTIdent(pub StringId);
#[derive(Debug)]
pub struct SASTString(pub StringId);

impl From<SASTIdent> for StringId {
    fn from(value: SASTIdent) -> Self {
        value.0
    }
}
