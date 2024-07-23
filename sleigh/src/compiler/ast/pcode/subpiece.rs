use crate::compiler::ast::common::SASTIdent;
use crate::compiler::ast::Spanned;

#[derive(Debug)]
pub struct SASTPCodeSubpiece {
    pub target: Spanned<SASTPCodeSubpieceTarget>,
    pub size: Spanned<u16>,
}

#[derive(Debug)]
pub enum SASTPCodeSubpieceTarget {
    Ident(Spanned<SASTIdent>),
    Literal(Spanned<u64>),
}
