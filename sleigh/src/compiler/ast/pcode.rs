pub mod addressof;
pub mod bitrange;
pub mod deref;
pub mod expression;
pub mod invocation;
pub mod statement;
pub mod subpiece;
pub mod target_expression;

use crate::compiler::ast::common::SASTIdent;
use crate::compiler::ast::pcode::statement::SASTPCodeStatement;
use crate::compiler::ast::Spanned;

/// Each pcode statement may have an optional label
pub type SASTPCodeLabelledStatement = (Option<Spanned<SASTIdent>>, Spanned<SASTPCodeStatement>);

#[derive(Debug)]
pub struct SASTPCode {
    pub statements: Vec<Spanned<SASTPCodeLabelledStatement>>,
}
