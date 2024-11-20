use crate::compiler::ast::Spanned;
use crate::compiler::ast::common::SASTIdent;
use crate::compiler::ast::pcode::expression::SASTPCodeExpression;

#[derive(Debug, Clone)]
pub struct SASTPCodeInvocation {
    pub name: Spanned<SASTIdent>,
    pub arguments: Vec<Spanned<SASTPCodeExpression>>,
}
