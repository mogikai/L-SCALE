use crate::compiler::ast::Spanned;

#[derive(Debug)]
pub struct SASTDisassemblySection<'src> {
    pub actions: Spanned<'src, &'src str>,
}
