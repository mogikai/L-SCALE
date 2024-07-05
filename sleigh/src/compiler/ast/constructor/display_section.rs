use crate::compiler::ast::Spanned;

#[derive(Debug)]
pub struct SASTDisplaySection<'src> {
    pub mnemonic: Spanned<'src, String>,
    pub identifiers: Vec<Spanned<'src, &'src str>>,
    pub display_str: Spanned<'src, String>,
}
