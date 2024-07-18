use chumsky::{
    IterParser,
    Parser,
};

use crate::compiler::ast::define::SASTRegister;
use crate::compiler::ast::Spanned;
use crate::compiler::lexer::ParserInput;
use crate::compiler::parser::helpers::spanned;
use crate::compiler::parser::{
    sast_ident,
    sast_ident_spanned,
    sleigh_control,
    sleigh_ident,
    sleigh_keyword,
    sleigh_number,
    sleigh_operation,
    sleigh_whitespace,
    whitespace_padded,
    SleighParserExtra,
};
use crate::compiler::tokens::{
    SleighControlCharacter,
    SleighKeyword,
    SleighOperation,
    SleighToken,
};

pub(super) fn register<'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTRegister>, SleighParserExtra<'tokens>> {
    let stringlist = sleigh_control(SleighControlCharacter::OpenBracket)
        .ignore_then(
            whitespace_padded(sast_ident().map_with(spanned))
                .repeated()
                .collect::<Vec<_>>(),
        )
        .then_ignore(sleigh_control(SleighControlCharacter::CloseBracket));

    let offset = sleigh_keyword(SleighKeyword::Offset)
        .ignore_then(sleigh_operation(SleighOperation::Equals))
        .ignore_then(sleigh_number())
        .map_with(spanned);

    let size = sleigh_keyword(SleighKeyword::Size)
        .ignore_then(sleigh_operation(SleighOperation::Equals))
        .ignore_then(sleigh_number())
        .map_with(spanned);

    whitespace_padded(sast_ident_spanned())
        .then(whitespace_padded(offset))
        .then(whitespace_padded(size))
        .then(stringlist)
        .map(|(((spacename, offset), size), regnames)| {
            SASTRegister {
                spacename,
                offset,
                size,
                regnames,
            }
        })
        .map_with(spanned)
}
