use crate::compiler::ast::pcode::SASTPCode;
use crate::compiler::ast::Spanned;

#[derive(Debug)]
pub struct SASTSemanticSection(pub Spanned<SASTPCode>);
