use crate::compiler::ast::Spanned;

#[derive(Debug)]
pub struct SASTBitPatternSection<'src> {
    pub constraints: Spanned<'src, &'src str>,
}
