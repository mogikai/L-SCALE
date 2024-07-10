use crate::compiler::ast::common::SASTIdent;
use crate::compiler::ast::Spanned;

#[derive(Debug)]
pub struct SASTPCodeSubpiece<'src> {
    pub ident: Spanned<'src, SASTIdent<'src>>,
    pub size: Spanned<'src, u16>,
}
