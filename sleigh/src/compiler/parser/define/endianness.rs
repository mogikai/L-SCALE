use chumsky::{error::Rich, extra, primitive::just, select, Parser};

use crate::compiler::{ast::{define::SASTEndianness, Spanned}, lexer::{InputSpan, ParserInput}, parser::{sleigh_keyword, sleigh_operation, SleighParserExtra}, tokens::{SleighControlCharacter, SleighKeyword, SleighOperation, SleighToken}};

pub(super) fn endianness<'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTEndianness>, SleighParserExtra<'tokens>> {
    let big_little = select! {
        SleighToken::Keyword(SleighKeyword::Big) => SASTEndianness::Big,
        SleighToken::Keyword(SleighKeyword::Little) => SASTEndianness::Little,
    };

    sleigh_keyword(SleighKeyword::Endian)
        .then(sleigh_operation(SleighOperation::Equals))
        .ignore_then(big_little)
        .map_with(|endian, extra| (endian, extra.span()))
}
