use crate::compiler::ast::item::SASTItem;
use crate::compiler::lexer::InputSpan;

pub mod attach;
pub mod common;
pub mod constructor;
pub mod define;
pub mod item;
pub mod pcode;
pub mod pcode_macro;

#[derive(Debug)]
pub struct SleighAST {
    pub(crate) items: Vec<SASTItem>,
}

pub type Spanned<T> = (T, InputSpan);
