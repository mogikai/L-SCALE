use crate::Spanned;
use crate::ast::pcode::SASTPCode;

#[derive(Debug)]
pub struct SASTSemanticSection(pub Spanned<SASTPCode>);
