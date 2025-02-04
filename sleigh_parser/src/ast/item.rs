use super::attach::SASTAttach;
use crate::Spanned;
use crate::ast::constructor::SASTConstructor;
use crate::ast::define::SASTDefine;
use crate::ast::pcode_macro::SASTPCodeMacro;
use crate::ast::with::SASTWith;
use crate::lexer::InputSpan;

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
