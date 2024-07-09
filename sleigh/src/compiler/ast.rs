use crate::compiler::ast::item::SASTItem;
use crate::compiler::lexer::InputSpan;

pub mod common;
pub mod constructor;
pub mod item;
pub mod pcode;
pub mod pcode_macro;

#[derive(Debug)]
pub struct SleighAST<'src> {
    pub(crate) items: Vec<SASTItem<'src>>,
}

pub type Spanned<'src, T> = (T, InputSpan<'src>);
