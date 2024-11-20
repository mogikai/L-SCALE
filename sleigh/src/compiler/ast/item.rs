use super::attach::SASTAttach;
use crate::compiler::ast::Spanned;
use crate::compiler::ast::constructor::SASTConstructor;
use crate::compiler::ast::define::SASTDefine;
use crate::compiler::ast::pcode_macro::SASTPCodeMacro;
use crate::compiler::ast::with::SASTWith;
use crate::compiler::lexer::InputSpan;

#[derive(Debug)]
pub enum SASTItem {
    Attach(Spanned<SASTAttach>),
    Constructor(Spanned<SASTConstructor>),
    Define(Spanned<SASTDefine>),
    Macro(Spanned<SASTPCodeMacro>),
    With(Spanned<SASTWith>),
    Error(InputSpan),
}

impl SASTItem {
    pub fn span(&self) -> &InputSpan {
        match self {
            SASTItem::Attach(attach) => attach.span(),
            SASTItem::Constructor(constructor) => constructor.span(),
            SASTItem::Define(define) => define.span(),
            SASTItem::Macro(pcode) => pcode.span(),
            SASTItem::With(with) => with.span(),
            SASTItem::Error(span) => span,
        }
    }
}
