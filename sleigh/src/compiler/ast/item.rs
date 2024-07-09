use crate::compiler::ast::constructor::SASTConstructor;
use crate::compiler::ast::pcode_macro::SASTPCodeMacro;
use crate::compiler::ast::Spanned;

#[derive(Debug)]
pub enum SASTItem<'src> {
    Constructor(Spanned<'src, SASTConstructor<'src>>),
    Macro(Spanned<'src, SASTPCodeMacro<'src>>),
}
