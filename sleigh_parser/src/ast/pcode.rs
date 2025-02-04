use crate::Spanned;
use crate::ast::common::SASTIdent;
use crate::ast::pcode::statement::SASTPCodeStatement;

pub mod addressof;
pub mod bitrange;
pub mod deref;
pub mod expression;
pub mod invocation;
pub mod statement;
pub mod subpiece;
pub mod target_expression;

/// Each pcode statement may have an optional label
pub type SASTPCodeLabelledStatement = (Option<Spanned<SASTIdent>>, Spanned<SASTPCodeStatement>);

#[derive(Debug, Clone)]
pub struct SASTPCode {
    pub statements: Vec<Spanned<SASTPCodeLabelledStatement>>,
}
