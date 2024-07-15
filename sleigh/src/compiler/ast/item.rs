use crate::compiler::ast::constructor::SASTConstructor;
use crate::compiler::ast::define::SASTDefine;
use crate::compiler::ast::pcode_macro::SASTPCodeMacro;
use crate::compiler::ast::Spanned;


#[derive(Debug)]
pub enum SASTItem {
    Constructor(Spanned<SASTConstructor>),
    Define(Spanned<SASTDefine>),
    Macro(Spanned<SASTPCodeMacro>),
}
