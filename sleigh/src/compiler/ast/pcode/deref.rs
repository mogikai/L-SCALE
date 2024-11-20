use crate::compiler::ast::Spanned;
use crate::compiler::ast::common::{
    SASTIdent,
    SASTSize,
};
use crate::compiler::ast::pcode::expression::SASTPCodeExpression;

/// `*v1` load data stored at v1
/// `*[space]v1` load data stored at v1 in space
/// `*:2 v1` load 2 byte data stored at v1
/// `*[space]:2 v1` load 2 byte data stored at v1 in space
#[derive(Debug, Clone)]
pub struct SASTPCodeDeref {
    /// The value to dereference. This loads the value stored at the pointer resulting from the expression
    pub expression: Box<Spanned<SASTPCodeExpression>>,
    /// Specifies the space to which the varnode is referring
    pub space: Option<Spanned<SASTIdent>>,
    /// The size in bytes of the value to load.
    pub size: Option<Spanned<SASTSize>>,
}
