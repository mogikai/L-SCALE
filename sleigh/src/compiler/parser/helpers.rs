use std::fmt::{
    Debug,
    Display,
};

use chumsky::input::MapExtra;

use crate::compiler::lexer::{
    InputSpan,
    ParserInput,
};
use crate::compiler::parser::SleighParserExtra;

pub(super) fn spanned<'tokens, T>(
    element: T,
    extra: &mut MapExtra<'tokens, '_, ParserInput<'tokens>, SleighParserExtra<'tokens>>,
) -> (T, InputSpan)
where
    T: Debug,
{
    (element, extra.span())
}
