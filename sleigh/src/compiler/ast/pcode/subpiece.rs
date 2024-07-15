use crate::compiler::ast::common::SASTIdent;
use crate::compiler::ast::Spanned;

#[derive(Debug)]
pub struct SASTPCodeSubpiece {
    pub ident: Spanned<SASTIdent>,
    pub size: Spanned<u16>,
}
