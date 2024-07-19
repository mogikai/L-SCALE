use chumsky::primitive::choice;
use chumsky::Parser;

use crate::compiler::ast::attach::{
    SASTAttach,
    SASTValue,
    SASTVariable,
};
use crate::compiler::ast::Spanned;
use crate::compiler::lexer::ParserInput;
use crate::compiler::tokens::{
    SleighControlCharacter,
    SleighKeyword,
};

use self::value::value;
use self::variable::variable;

use super::{
    sleigh_control,
    sleigh_keyword,
    sleigh_whitespace,
    SleighParserExtra,
};

mod value;
mod variable;

pub(super) fn attach<'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTAttach>, SleighParserExtra<'tokens>> {
    let variable = variable().map(SASTAttach::SASTVariable);
    let value = value().map(SASTAttach::SASTValue);

    let attach = choice((variable, value));

    sleigh_keyword(SleighKeyword::Attach)
        .padded_by(sleigh_whitespace())
        .ignore_then(attach)
        .then_ignore(sleigh_control(SleighControlCharacter::SemiColon))
        .map_with(|attach, extra| (attach, extra.span()))
        .boxed()
}
