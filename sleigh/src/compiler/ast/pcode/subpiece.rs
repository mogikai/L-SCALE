use crate::compiler::ast::Spanned;
use crate::compiler::ast::common::{
    SASTIdent,
    SASTLiteral,
    SASTSize,
};

#[derive(Debug, Clone)]
pub struct SASTPCodeSubpiece {
    pub target: Spanned<SASTPCodeSubpieceTarget>,
    pub selector: Spanned<SASTPCodeSubpieceSelector>,
}

#[derive(Debug, Clone)]
pub enum SASTPCodeSubpieceSelector {
    /// Truncate to the desired size, only keeping the specified number of bytes starting from LSB.
    Take(SASTSize),
    /// Discard the desired number of bytes starting at LSB, keeping the rest.
    Discard(SASTSize),
}

#[derive(Debug, Clone)]
pub enum SASTPCodeSubpieceTarget {
    Ident(Spanned<SASTIdent>),
    Literal(Spanned<SASTLiteral>),
}
