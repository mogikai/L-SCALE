use chumsky::{error::Rich, extra, primitive::just, Parser};

use crate::compiler::{ast::Spanned, lexer::{InputSpan, ParserInput}, parser::{sleigh_keyword, sleigh_number, sleigh_operation, SleighParserExtra}, tokens::{SleighControlCharacter, SleighKeyword, SleighOperation, SleighToken}};

pub(super) fn alignment<'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<usize>, SleighParserExtra<'tokens>> {
    sleigh_keyword(SleighKeyword::Alignment)
        .then(sleigh_operation(SleighOperation::Equals))
        .ignore_then(sleigh_number())
        .map_with(|alignment, extra| (alignment as usize, extra.span()))
}
