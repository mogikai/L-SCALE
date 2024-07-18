use crate::compiler::ast::constructor::SASTConstructor;
use crate::compiler::ast::define::SASTDefine;
use crate::compiler::ast::pcode_macro::SASTPCodeMacro;
use crate::compiler::ast::Spanned;

use super::attach::SASTAttach;

#[derive(Debug)]
pub enum SASTItem {
    Attach(Spanned<SASTAttach>),
    Constructor(Spanned<SASTConstructor>),
    Define(Spanned<SASTDefine>),
    Macro(Spanned<SASTPCodeMacro>),
}
