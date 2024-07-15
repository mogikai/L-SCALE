pub mod bitrange;
pub mod expression;
pub mod invocation;
pub mod statement;
pub mod subpiece;
pub mod target_expression;

use crate::compiler::ast::pcode::statement::SASTPCodeStatement;
use crate::compiler::ast::Spanned;

#[derive(Debug)]
pub struct SASTPCode {
    pub statements: Vec<Spanned<SASTPCodeStatement>>,
}
