use crate::compiler::ast::Spanned;
use crate::compiler::ast::common::{
    SASTIdent,
    SASTSize,
};

#[derive(Debug, Clone)]
pub struct SASTPCodeBitRange {
    pub ident: Spanned<SASTIdent>,
    pub lsb: Spanned<SASTSize>,
    pub bits: Spanned<SASTSize>,
}
