use crate::compiler::ast::common::SASTIdent;
use crate::compiler::ast::pcode::expression::SASTPCodeExpression;
use crate::compiler::ast::Spanned;

#[derive(Debug)]
pub enum SASTPCodeStatement<'src> {
    Assignment {
        local: Spanned<'src, bool>,
        target: Spanned<'src, SASTIdent<'src>>,
        expression: Spanned<'src, SASTPCodeExpression<'src>>,
    },
    Declaration(Spanned<'src, SASTIdent<'src>>),
}
