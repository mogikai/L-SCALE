use chumsky::{
    IterParser,
    Parser,
};

use crate::compiler::ast::define::SASTRegister;
use crate::compiler::ast::Spanned;
use crate::compiler::lexer::ParserInput;
use crate::compiler::parser::{
    sast_ident,
    sast_ident_spanned,
    sleigh_control,
    sleigh_ident,
    sleigh_keyword,
    sleigh_number,
    sleigh_operation,
    sleigh_whitespace,
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
            sast_ident()
                .map_with(|ident, extra| (ident, extra.span()))
                .padded_by(sleigh_whitespace())
                .repeated()
                .collect::<Vec<_>>(),
        )
        .then_ignore(sleigh_control(SleighControlCharacter::CloseBracket));

    let offset = sleigh_keyword(SleighKeyword::Offset)
        .ignore_then(sleigh_operation(SleighOperation::Equals))
        .ignore_then(sleigh_number())
        .map_with(|num, extra| (num, extra.span()));

    let size = sleigh_keyword(SleighKeyword::Size)
        .ignore_then(sleigh_operation(SleighOperation::Equals))
        .ignore_then(sleigh_number())
        .map_with(|num, extra| (num, extra.span()));

    sast_ident_spanned()
        .padded_by(sleigh_whitespace())
        .then(offset)
        .padded_by(sleigh_whitespace())
        .then(size)
        .padded_by(sleigh_whitespace())
        .then(stringlist)
        .map_with(|(((spacename, offset), size), strings), extra| {
            eprintln!("value = {:#?}", offset);

            (
                SASTRegister {
                    spacename,
                    offset,
                    size,
                    regnames: strings,
                },
                extra.span(),
            )
        })
}
