use crate::compiler::ast::common::{SASTIdent, SASTSize};
use crate::compiler::ast::Spanned;

#[derive(Debug)]
pub struct SASTPCodeBitRange {
    pub ident: Spanned<SASTIdent>,
    pub lsb: Spanned<SASTSize>,
    pub bits: Spanned<SASTSize>,
}
