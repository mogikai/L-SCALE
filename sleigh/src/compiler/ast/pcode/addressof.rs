use crate::compiler::ast::common::SASTIdent;
use crate::compiler::ast::pcode::expression::SASTPCodeExpression;
use crate::compiler::ast::Spanned;

#[derive(Debug)]
pub struct SASTPCodeAddressOf {
    /// The value to take the address of.
    pub expression: Box<Spanned<SASTPCodeExpression>>,
    /// The size in bytes of the value to take the address of.
    pub size: Option<Spanned<u16>>,
}
