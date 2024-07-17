use chumsky::Parser;

use crate::compiler::ast::Spanned;
use crate::compiler::lexer::ParserInput;
use crate::compiler::parser::helpers::spanned;
use crate::compiler::parser::{
    sleigh_keyword,
    sleigh_number,
    sleigh_operation,
    SleighParserExtra,
};
use crate::compiler::tokens::{
    SleighKeyword,
    SleighOperation,
};

pub(super) fn alignment<'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<u64>, SleighParserExtra<'tokens>> {
    sleigh_keyword(SleighKeyword::Alignment)
        .then(sleigh_operation(SleighOperation::Equals))
        .ignore_then(sleigh_number())
        .map_with(spanned)
}
