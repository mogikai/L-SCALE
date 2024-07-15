use crate::compiler::ast::common::SASTIdent;
use crate::compiler::ast::pcode::SASTPCode;
use crate::compiler::ast::Spanned;

#[derive(Debug)]
pub struct SASTPCodeMacro {
    pub ident: Spanned<SASTIdent>,
    pub arguments: Spanned<Vec<Spanned<SASTIdent>>>,
    pub pcode: Spanned<SASTPCode>,
}
