use crate::Spanned;
use crate::lexer::ParserInput;
use crate::parser::helpers::spanned;
use crate::parser::{
    SleighParserExtra,
    sleigh_keyword,
    sleigh_number,
    sleigh_operation,
};
use crate::tokens::{
    SleighKeyword,
    SleighOperation,
};
use chumsky::Parser;
use std::fmt::{
    Debug,
    Display,
};

pub fn alignment<'tokens, T>() -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<T>, SleighParserExtra<'tokens>>
where
    T: TryFrom<u64> + Debug,
    <T as TryFrom<u64>>::Error: Display,
{
    sleigh_keyword(SleighKeyword::Alignment)
        .then(sleigh_operation(SleighOperation::Equals))
        .ignore_then(sleigh_number::<T>())
        .map_with(spanned)
}
