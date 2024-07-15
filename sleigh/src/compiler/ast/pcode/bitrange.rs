use crate::compiler::ast::common::SASTIdent;
use crate::compiler::ast::Spanned;

#[derive(Debug)]
pub struct SASTBitRange {
    ident: Spanned<SASTIdent>,
    lsb: Spanned<u16>,
    bits: Spanned<u16>,
}
