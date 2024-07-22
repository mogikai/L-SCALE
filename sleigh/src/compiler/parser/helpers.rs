use crate::compiler::lexer::{
    InputSpan,
    ParserInput,
};
use crate::compiler::parser::SleighParserExtra;
use chumsky::input::MapExtra;
use std::fmt::Debug;
use tracing::trace;

pub(super) fn spanned<'tokens, T>(
    element: T,
    extra: &mut MapExtra<'tokens, '_, ParserInput<'tokens>, SleighParserExtra<'tokens>>,
) -> (T, InputSpan)
where
    T: Debug,
{
    (element, extra.span())
}
