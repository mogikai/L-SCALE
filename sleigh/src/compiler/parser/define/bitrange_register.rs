use chumsky::{IterParser, Parser};

use crate::compiler::{ast::{define::{SASTBitrangeRegister, SASTRegister}, Spanned}, lexer::ParserInput, parser::{sast_ident, sast_ident_spanned, sleigh_control, sleigh_ident, sleigh_keyword, sleigh_number, sleigh_operation, sleigh_whitespace, SleighParserExtra}, tokens::{SleighControlCharacter, SleighKeyword, SleighOperation, SleighToken}};

pub(super) fn bitrange_register<'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Vec<Spanned<SASTBitrangeRegister>>, SleighParserExtra<'tokens>> {
    let number_pair = sleigh_control(SleighControlCharacter::OpenBracket)
        .ignore_then(sleigh_number())
        .then(sleigh_control(SleighControlCharacter::Comma))
        .then(sleigh_number())
        .then_ignore(sleigh_control(SleighControlCharacter::CloseBracket))
        .map(|((offset, _), size)| (offset as usize, size as usize));

    let register = sast_ident_spanned() // bitrange register name
        .then(sleigh_operation(SleighOperation::Equals))
        .then(sast_ident_spanned()) // source register
        .then(number_pair)
        .map_with(|(((name, _), source_register), (offset, size)), extra| {
            (SASTBitrangeRegister {
                name,
                source_register,
                offset,
                size
            }, extra.span())
        });

    sleigh_keyword(SleighKeyword::BitRange)
        .then(sleigh_whitespace())
        .ignore_then(register
            .padded_by(sleigh_whitespace())
            .repeated()
            .collect::<Vec<_>>()
        )
}
