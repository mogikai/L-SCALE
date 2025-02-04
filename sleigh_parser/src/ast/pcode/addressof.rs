use crate::Spanned;
use crate::ast::common::SASTSize;
use crate::ast::pcode::expression::SASTPCodeExpression;

#[derive(Debug, Clone)]
pub struct SASTPCodeAddressOf {
    /// The value to take the address of.
    pub expression: Box<Spanned<SASTPCodeExpression>>,
    /// The size in bytes of the value to take the address of.
    pub size: Option<Spanned<SASTSize>>,
}
