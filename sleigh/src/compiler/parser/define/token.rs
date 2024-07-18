use chumsky::primitive::choice;
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
use crate::compiler::parser::{
    sast_ident_spanned,
    sleigh_control,
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

use super::endianness::endianness;

pub(super) fn token<'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTToken>, SleighParserExtra<'tokens>> {
    let open_paren = sleigh_control(SleighControlCharacter::OpenParen);
    let close_paren = sleigh_control(SleighControlCharacter::CloseParen);

    let endianness = endianness();

    let number_pair = sleigh_number()
        .map_with(|num, extra| (num, extra.span()))
        .then(sleigh_control(SleighControlCharacter::Comma))
        .then(sleigh_number().map_with(|num, extra| (num, extra.span())))
        .delimited_by(open_paren, close_paren)
        .map(|((start, _), end)| (start, end));

    let attribute = select! {
        SleighToken::Keyword(SleighKeyword::Signed) => SASTTokenFieldAttribute::Signed,
        SleighToken::Keyword(SleighKeyword::Hex) => SASTTokenFieldAttribute::Hex,
        SleighToken::Keyword(SleighKeyword::Dec) => SASTTokenFieldAttribute::Dec,
    };

    let attributelist = attribute
        .map_with(|attr, extra| (attr, extra.span()))
        .padded_by(sleigh_whitespace())
        .repeated()
        .collect::<Vec<_>>();

    let field = sast_ident_spanned()
        .then(sleigh_operation(SleighOperation::Equals))
        .then(number_pair)
        .padded_by(sleigh_whitespace())
        .then(attributelist)
        .map_with(|(((name, _), (start, end)), attributes), extra| {
            (
                SASTTokenField {
                    name,
                    start,
                    end,
                    attributes,
                },
                extra.span(),
            )
        });

    sleigh_keyword(SleighKeyword::Token)
        .padded_by(sleigh_whitespace())
        .ignore_then(sast_ident_spanned())
        .padded_by(sleigh_whitespace())
        .then(
            sleigh_number()
                .delimited_by(open_paren, close_paren)
                .map_with(|num, extra| (num, extra.span())),
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
        .map_with(|(((name, bits), endianness), fields), extra| (SASTToken { name, bits, fields }, extra.span()))
}
