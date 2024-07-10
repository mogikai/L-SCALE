use crate::compiler::ast::common::SASTIdent;
use crate::compiler::ast::pcode::expression::SASTPCodeExpression;
use crate::compiler::ast::Spanned;

#[derive(Debug)]
pub struct SASTPCodeInvocation<'src> {
    pub name: Spanned<'src, SASTIdent<'src>>,
    pub arguments: Vec<Spanned<'src, SASTPCodeExpression<'src>>>,
}
