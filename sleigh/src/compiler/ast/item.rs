use super::attach::SASTAttach;
use crate::compiler::ast::constructor::SASTConstructor;
use crate::compiler::ast::define::SASTDefine;
use crate::compiler::ast::pcode_macro::SASTPCodeMacro;
use crate::compiler::ast::Spanned;
use crate::compiler::lexer::InputSpan;

#[derive(Debug)]
pub enum SASTItem {
    Attach(Spanned<SASTAttach>),
    Constructor(Spanned<SASTConstructor>),
    Define(Spanned<SASTDefine>),
    Macro(Spanned<SASTPCodeMacro>),
    Error(InputSpan),
}
