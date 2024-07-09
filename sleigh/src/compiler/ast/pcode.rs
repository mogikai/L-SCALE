use crate::compiler::ast::Spanned;

#[derive(Debug)]
pub struct SASTPCode<'src> {
    pub expressions: Vec<Spanned<'src, &'src str>>,
}
