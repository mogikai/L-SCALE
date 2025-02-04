use crate::Spanned;
use crate::ast::define::SASTEndianness;
use crate::lexer::ParserInput;
use crate::parser::helpers::spanned;
use crate::parser::{
    SleighParserExtra,
    sleigh_keyword,
    sleigh_operation,
    whitespace_padded,
};
use crate::tokens::{
    SleighKeyword,
    SleighOperation,
    SleighToken,
};
use chumsky::{
    Parser,
    select,
};

pub fn endianness<'tokens>()
-> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTEndianness>, SleighParserExtra<'tokens>> {
    let big_little = select! {
        SleighToken::Keyword(SleighKeyword::Big) => SASTEndianness::Big,
        SleighToken::Keyword(SleighKeyword::Little) => SASTEndianness::Little,
    };

    sleigh_keyword(SleighKeyword::Endian)
        .then(whitespace_padded(sleigh_operation(SleighOperation::Equals)))
        .ignore_then(big_little)
        .map_with(spanned)
}
