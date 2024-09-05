use chumsky::{
    select,
    IterParser,
    Parser,
};

use crate::compiler::ast::define::{
    SASTToken,
    SASTTokenField,
    SASTTokenFieldAttribute,
};
use crate::compiler::ast::Spanned;
use crate::compiler::lexer::ParserInput;
use crate::compiler::parser::helpers::spanned;
use crate::compiler::parser::{
    sast_ident_spanned,
    sleigh_control,
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

use super::endianness::endianness;

pub(super) fn token<'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTToken>, SleighParserExtra<'tokens>> {
    let endianness = endianness();

    let number_pair = sleigh_number()
        .map_with(spanned)
        .then(sleigh_control(SleighControlCharacter::Comma))
        .then(sleigh_number().map_with(spanned))
        .delimited_by(
            sleigh_control(SleighControlCharacter::OpenParen),
            sleigh_control(SleighControlCharacter::CloseParen),
        )
        .map(|((start, _), end)| (start, end))
        .boxed();

    let attribute = select! {
        SleighToken::Keyword(SleighKeyword::Signed) => SASTTokenFieldAttribute::Signed,
        SleighToken::Keyword(SleighKeyword::Hex) => SASTTokenFieldAttribute::Hex,
        SleighToken::Keyword(SleighKeyword::Dec) => SASTTokenFieldAttribute::Dec,
    };

    let attributelist = attribute
        .map_with(spanned)
        .padded_by(sleigh_whitespace())
        .repeated()
        .collect::<Vec<_>>()
        .boxed();

    let field = sast_ident_spanned()
        .then(whitespace_padded(sleigh_operation(SleighOperation::Equals)))
        .then(number_pair)
        .padded_by(sleigh_whitespace())
        .then(attributelist)
        .map(|(((name, _), (start, end)), attributes)| SASTTokenField {
            name,
            start,
            end,
            attributes,
        })
        .map_with(spanned)
        .boxed();

    sleigh_keyword(SleighKeyword::Token)
        .padded_by(sleigh_whitespace())
        .ignore_then(sast_ident_spanned())
        .padded_by(sleigh_whitespace())
        .then(
            sleigh_number()
                .delimited_by(
                    sleigh_control(SleighControlCharacter::OpenParen),
                    sleigh_control(SleighControlCharacter::CloseParen),
                )
                .map_with(spanned),
        )
        .padded_by(sleigh_whitespace())
        .then(endianness.or_not())
        .padded_by(sleigh_whitespace())
        .then(
            field
                .padded_by(sleigh_whitespace())
                .repeated()
                .collect::<Vec<_>>(),
        )
        .map(|(((name, bits), endian_override), fields)| SASTToken {
            name,
            bits,
            endian_override,
            fields,
        })
        .map_with(spanned)
        .boxed()
}
