use crate::Spanned;
use crate::ast::common::SASTIdent;
use crate::ast::pcode::SASTPCode;

#[derive(Debug)]
pub struct SASTPCodeMacro {
    pub ident: Spanned<SASTIdent>,
    pub arguments: Spanned<Vec<Spanned<SASTIdent>>>,
    pub pcode: Spanned<SASTPCode>,
}
