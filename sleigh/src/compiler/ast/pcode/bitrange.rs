use crate::compiler::ast::common::SASTIdent;
use crate::compiler::ast::Spanned;

#[derive(Debug)]
pub struct SASTPCodeBitRange {
    pub ident: Spanned<SASTIdent>,
    pub lsb: Spanned<u16>,
    pub bits: Spanned<u16>,
}
