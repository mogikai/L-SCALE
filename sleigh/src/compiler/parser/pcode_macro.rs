use chumsky::error::Rich;
use chumsky::primitive::just;
use chumsky::{
    extra,
    IterParser,
    Parser,
};

use crate::compiler::ast::pcode_macro::SASTPCodeMacro;
use crate::compiler::ast::Spanned;
use crate::compiler::lexer::{
    InputSpan,
    ParserInput,
};
use crate::compiler::parser::pcode::pcode;
use crate::compiler::parser::{
    parenthesized,
    sast_ident,
    whitespace_padded,
};
use crate::compiler::tokens::{
    SleighControlCharacter,
    SleighKeyword,
    SleighToken,
};

pub(super) fn pcode_macro<'tokens, 'src: 'tokens>() -> impl Parser<
    'tokens,
    ParserInput<'tokens, 'src>,
    Spanned<'src, SASTPCodeMacro<'src>>,
    extra::Err<Rich<'tokens, SleighToken<'src>, InputSpan<'src>>>,
> {
    let ident = whitespace_padded(sast_ident().map_with(|ident, extra| (ident, extra.span())));

    let argument = whitespace_padded(sast_ident().map_with(|ident, extra| (ident, extra.span())));

    let arguments = parenthesized(
        argument
            .separated_by(just(SleighToken::Control(SleighControlCharacter::Comma)))
            .collect::<Vec<_>>(),
    )
    .map_with(|val, extra| (val, extra.span()));

    let pcode = whitespace_padded(pcode().delimited_by(
        just(SleighToken::Control(SleighControlCharacter::OpenBrace)),
        just(SleighToken::Control(SleighControlCharacter::CloseBrace)),
    ));

    just(SleighToken::Keyword(SleighKeyword::Macro))
        .ignore_then(ident)
        .then(arguments)
        .then(pcode)
        .map_with(|((ident, arguments), pcode), extra| {
            (
                SASTPCodeMacro {
                    ident,
                    arguments,
                    pcode,
                },
                extra.span(),
            )
        })
}
