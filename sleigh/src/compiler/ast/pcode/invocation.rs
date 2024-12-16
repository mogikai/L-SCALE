use crate::compiler::ast::common::SASTIdent;
use crate::compiler::ast::pcode::expression::SASTPCodeExpression;
use crate::compiler::ast::Spanned;

#[derive(Debug, Clone)]
pub struct SASTPCodeInvocation {
    pub name: Spanned<SASTIdent>,
    pub arguments: Vec<Spanned<SASTPCodeExpression>>,
}
