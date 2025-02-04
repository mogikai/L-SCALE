use std::fmt::Debug;

use crate::Spanned;
use crate::lexer::ParserInput;
use crate::parser::SleighParserExtra;
use chumsky::input::MapExtra;

pub fn spanned<'tokens, T>(
    element: T,
    extra: &mut MapExtra<'tokens, '_, ParserInput<'tokens>, SleighParserExtra<'tokens>>,
) -> Spanned<T>
where
    T: Debug,
{
    Spanned::new(element, extra.span())
}
