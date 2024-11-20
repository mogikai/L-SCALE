use crate::compiler::ast::Spanned;
use crate::compiler::ast::pcode::SASTPCode;

#[derive(Debug)]
pub struct SASTSemanticSection(pub Spanned<SASTPCode>);
