use chumsky::{
    select,
    IterParser,
    Parser,
};

use crate::compiler::ast::define::{
    SASTContext,
    SASTContextField,
    SASTContextFieldAttribute,
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

pub(super) fn context<'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTContext>, SleighParserExtra<'tokens>> {
    let number_pair = sleigh_number()
        .map_with(spanned)
        .then(sleigh_control(SleighControlCharacter::Comma))
        .then(sleigh_number().map_with(spanned))
        .delimited_by(
            sleigh_control(SleighControlCharacter::OpenParen),
            sleigh_control(SleighControlCharacter::CloseParen),
        )
        .map(|((start, _), end)| (start, end));

    let attribute = select! {
        SleighToken::Keyword(SleighKeyword::Signed) => SASTContextFieldAttribute::Signed,
        SleighToken::Keyword(SleighKeyword::Hex) => SASTContextFieldAttribute::Hex,
        SleighToken::Keyword(SleighKeyword::Dec) => SASTContextFieldAttribute::Dec,
        SleighToken::Keyword(SleighKeyword::Noflow) => SASTContextFieldAttribute::Noflow,
    };

    let attributelist = whitespace_padded(attribute.map_with(spanned))
        .repeated()
        .collect::<Vec<_>>();

    let field = sast_ident_spanned()
        .then(whitespace_padded(sleigh_operation(SleighOperation::Equals)))
        .then(number_pair)
        .then(attributelist)
        .map(|(((name, _), (start, end)), attributes)| SASTContextField {
            name,
            start,
            end,
            attributes,
        })
        .map_with(spanned)
        .boxed();

    sleigh_keyword(SleighKeyword::Context)
        .padded_by(sleigh_whitespace())
        .ignore_then(sast_ident_spanned())
        .padded_by(sleigh_whitespace())
        .then(
            field
                .padded_by(sleigh_whitespace())
                .repeated()
                .collect::<Vec<_>>(),
        )
        .map(|(reg, fields)| SASTContext { varnode: reg, fields })
        .map_with(spanned)
        .boxed()
}
