use chumsky::error::Rich;
use chumsky::prelude::{
    any,
    just,
};
use chumsky::{
    extra,
    IterParser,
    Parser,
};

use crate::compiler::ast::constructor::display_section::SASTDisplaySection;
use crate::compiler::ast::Spanned;
use crate::compiler::lexer::{
    InputSpan,
    ParserInput,
};
use crate::compiler::parser::sleigh_ident;
use crate::compiler::tokens::{
    SleighControlCharacter,
    SleighKeyword,
    SleighOperation,
    SleighToken,
};

pub(super) fn display_section<'tokens, 'src: 'tokens>() -> impl Parser<
    'tokens,
    ParserInput<'tokens, 'src>,
    Spanned<'src, SASTDisplaySection<'src>>,
    extra::Err<Rich<'tokens, SleighToken<'src>, InputSpan<'src>>>,
> {
    let start_of_display_section = just(SleighToken::Control(SleighControlCharacter::Colon));
    let end_of_display_section = just(SleighToken::Keyword(SleighKeyword::Is));

    // Parse the display string but then rewind, so we can parse mnemonic and identifiers.
    let display = any()
        .and_is(end_of_display_section.not())
        .map(|tok: SleighToken| tok.to_mnemonic().to_string())
        .repeated()
        .collect::<Vec<String>>()
        .map_with(|s, extra| (s.into_iter().collect::<String>(), extra.span()))
        .rewind();

    // Parse mnemonic, removing any `^` and rewind.
    // If we encounter a whitespace in the beginning, the mnemonic is empty.
    let mnemonic = just(SleighToken::Whitespace)
        .ignored()
        .map_with(|_, extra| (String::new(), extra.span()))
        .or(any()
            .and_is(just(SleighToken::Whitespace).not())
            .then_ignore(just(SleighToken::Operation(SleighOperation::Hat)).or_not())
            .map(|token: SleighToken| token.to_mnemonic().to_string())
            .repeated()
            .collect::<Vec<_>>()
            .map_with(|element, extra| (element.into_iter().collect::<String>(), extra.span())))
        .rewind();

    let identifiers = sleigh_ident()
        .padded_by(
            any()
                .and_is(sleigh_ident().not())
                .and_is(end_of_display_section.not())
                .repeated(),
        )
        .map_with(|ident, extra| (ident, extra.span()))
        .repeated()
        .collect::<Vec<_>>();

    start_of_display_section
        .ignore_then(display)
        .then(mnemonic)
        .then(identifiers)
        // .then_ignore(any().and_is(end_of_display_section.not()).repeated())
        .then_ignore(end_of_display_section)
        .map_with(|((display_str, mnemonic), identifiers), extra| {
            (
                SASTDisplaySection {
                    mnemonic,
                    identifiers,
                    display_str,
                },
                extra.span(),
            )
        })
}
