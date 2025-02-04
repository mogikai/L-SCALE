use crate::Spanned;
use crate::ast::common::SASTIdent;
use crate::ast::pcode::expression::SASTPCodeExpression;

#[derive(Debug, Clone)]
pub struct SASTPCodeInvocation {
    pub name: Spanned<SASTIdent>,
    pub arguments: Vec<Spanned<SASTPCodeExpression>>,
}
