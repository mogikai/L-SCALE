use crate::compiler::ast::common::{SASTIdent, SASTLiteral, SASTSize};
use crate::compiler::ast::Spanned;

#[derive(Debug)]
pub struct SASTPCodeSubpiece {
    pub target: Spanned<SASTPCodeSubpieceTarget>,
    pub selector: Spanned<SASTPCodeSubpieceSelector>,
}

#[derive(Debug)]
pub enum SASTPCodeSubpieceSelector {
    /// Truncate to the desired size, only keeping the specified number of bytes starting from LSB.
    Take(SASTSize),
    /// Discard the desired number of bytes starting at LSB, keeping the rest.
    Discard(SASTSize),
}

#[derive(Debug)]
pub enum SASTPCodeSubpieceTarget {
    Ident(Spanned<SASTIdent>),
    Literal(Spanned<SASTLiteral>),
}
