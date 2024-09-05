use std::fmt::Debug;

use crate::compiler::ast::Spanned;
use crate::compiler::lexer::ParserInput;
use crate::compiler::parser::SleighParserExtra;
use chumsky::input::MapExtra;

pub(super) fn spanned<'tokens, T>(
    element: T,
    extra: &mut MapExtra<'tokens, '_, ParserInput<'tokens>, SleighParserExtra<'tokens>>,
) -> Spanned<T>
where
    T: Debug,
{
    Spanned::new(element, extra.span())
}
