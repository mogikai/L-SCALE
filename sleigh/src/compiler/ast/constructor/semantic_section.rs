use crate::compiler::ast::pcode::SASTPCode;
use crate::compiler::ast::Spanned;

#[derive(Debug)]
pub struct SASTSemanticSection<'src> {
    pub actions: Spanned<'src, SASTPCode<'src>>,
}
