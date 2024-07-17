use chumsky::{
    select,
    Parser,
};

use crate::compiler::ast::define::SASTEndianness;
use crate::compiler::ast::Spanned;
use crate::compiler::lexer::ParserInput;
use crate::compiler::parser::helpers::spanned;
use crate::compiler::parser::{
    sleigh_keyword,
    sleigh_operation,
    SleighParserExtra,
};
use crate::compiler::tokens::{
    SleighKeyword,
    SleighOperation,
    SleighToken,
};

pub(super) fn endianness<'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTEndianness>, SleighParserExtra<'tokens>> {
    let big_little = select! {
        SleighToken::Keyword(SleighKeyword::Big) => SASTEndianness::Big,
        SleighToken::Keyword(SleighKeyword::Little) => SASTEndianness::Little,
    };

    sleigh_keyword(SleighKeyword::Endian)
        .then(sleigh_operation(SleighOperation::Equals))
        .ignore_then(big_little)
        .map_with(spanned)
}
