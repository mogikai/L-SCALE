use crate::compiler::ast::common::SASTIdent;
use crate::compiler::ast::Spanned;

#[derive(Debug)]
pub struct SASTBitRange<'src> {
    ident: Spanned<'src, SASTIdent<'src>>,
    lsb: Spanned<'src, u16>,
    bits: Spanned<'src, u16>,
}
