use crate::compiler::lexer::{
    InputSpan,
    ParserInput,
};
use crate::compiler::parser::SleighParserExtra;
use chumsky::input::MapExtra;

pub(super) fn spanned<'tokens, T>(
    element: T,
    extra: &mut MapExtra<'tokens, '_, ParserInput<'tokens>, SleighParserExtra<'tokens>>,
) -> (T, InputSpan) {
    (element, extra.span())
}
