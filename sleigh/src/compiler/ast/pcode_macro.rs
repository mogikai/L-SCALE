use crate::compiler::ast::common::SASTIdent;
use crate::compiler::ast::pcode::SASTPCode;
use crate::compiler::ast::Spanned;

#[derive(Debug)]
pub struct SASTPCodeMacro<'src> {
    pub ident: Spanned<'src, SASTIdent<'src>>,
    pub arguments: Spanned<'src, Vec<Spanned<'src, SASTIdent<'src>>>>,
    pub pcode: Spanned<'src, SASTPCode<'src>>,
}
