pub mod expression;
pub mod statement;

use crate::compiler::ast::pcode::statement::SASTPCodeStatement;
use crate::compiler::ast::Spanned;

#[derive(Debug)]
pub struct SASTPCode<'src> {
    pub statements: Vec<Spanned<'src, SASTPCodeStatement<'src>>>,
}
