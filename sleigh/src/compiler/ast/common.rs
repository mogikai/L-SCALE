use crate::compiler::ast::Spanned;
use crate::compiler::StringId;

#[derive(Debug)]
pub struct SASTIdent(pub StringId);
#[derive(Debug)]
pub struct SASTString(pub StringId);

impl From<Spanned<SASTIdent>> for StringId {
    fn from(value: Spanned<SASTIdent>) -> Self {
        value.0 .0
    }
}
