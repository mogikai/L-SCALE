use chumsky::Parser;
use std::fmt::{
    Debug,
    Display,
};

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

pub(super) fn alignment<'tokens, T>()
-> impl Parser<'tokens, ParserInput<'tokens>, Spanned<T>, SleighParserExtra<'tokens>>
where
    T: TryFrom<u64> + Debug,
    <T as TryFrom<u64>>::Error: Display,
{
    sleigh_keyword(SleighKeyword::Alignment)
        .then(sleigh_operation(SleighOperation::Equals))
        .ignore_then(sleigh_number::<T>())
        .map_with(spanned)
}
