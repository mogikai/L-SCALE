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
use crate::compiler::parser::helpers::spanned;
use crate::compiler::parser::{
    sleigh_ident,
    SleighParserExtra,
};
use crate::compiler::tokens::{
    SleighControlCharacter,
    SleighKeyword,
    SleighOperation,
    SleighToken,
};

pub(super) fn display_section<'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTDisplaySection>, SleighParserExtra<'tokens>> {
    let start_of_display_section = just(SleighToken::Control(SleighControlCharacter::Colon));
    let end_of_display_section = just(SleighToken::Keyword(SleighKeyword::Is));

    // Parse the display string but then rewind, so we can parse mnemonic and identifiers.
    let display = any()
        .and_is(end_of_display_section.not())
        .map(|tok: SleighToken| tok.to_mnemonic().to_string())
        .repeated()
        .collect::<Vec<String>>()
        .map(|s| s.into_iter().collect::<String>())
        .map_with(spanned)
        .rewind();

    // Parse mnemonic, removing any `^` and rewind.
    // If we encounter a whitespace in the beginning, the mnemonic is empty.
    let mnemonic = just(SleighToken::Whitespace)
        .to(String::new())
        .map_with(spanned)
        .or(any()
            .and_is(just(SleighToken::Whitespace).not())
            .then_ignore(just(SleighToken::Operation(SleighOperation::Hat)).or_not())
            .map(|token: SleighToken| token.to_mnemonic().to_string())
            .repeated()
            .collect::<Vec<_>>()
            .map(|element| element.into_iter().collect::<String>())
            .map_with(spanned))
        .rewind();

    let identifiers = sleigh_ident()
        .padded_by(
            any()
                .and_is(sleigh_ident().not())
                .and_is(end_of_display_section.not())
                .repeated(),
        )
        .map_with(spanned)
        .repeated()
        .collect::<Vec<_>>();

    start_of_display_section
        .ignore_then(display)
        .then(mnemonic)
        .then(identifiers)
        // .then_ignore(any().and_is(end_of_display_section.not()).repeated())
        .then_ignore(end_of_display_section)
        .map(|((display_str, mnemonic), identifiers)| SASTDisplaySection {
            mnemonic,
            identifiers,
            display_str,
        })
        .map_with(spanned)
}
