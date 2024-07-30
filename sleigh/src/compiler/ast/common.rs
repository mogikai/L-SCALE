use crate::compiler::ast::Spanned;
use crate::compiler::StringId;

#[derive(Debug, Copy, Clone)]
pub struct SASTIdent(pub StringId);
#[derive(Debug)]
pub struct SASTString(pub StringId);

// impl From<Spanned<SASTIdent>> for Spanned<StringId> {
//     fn from(value: Spanned<SASTIdent>) -> Self {
//         (value.0.0, value.1)
//     }
// }
