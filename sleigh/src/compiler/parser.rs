use ariadne::{
    Label,
    Report,
    ReportKind,
    Source,
};
use chumsky::{
    extra,
    IterParser,
    Parser,
    select,
};
use chumsky::error::Rich;
use chumsky::prelude::{
    any,
    end,
    one_of,
    skip_then_retry_until,
};
use chumsky::primitive::just;
use chumsky::span::Span;
use tracing::trace;

use crate::compiler::ast::{
    SleighAST,
    Spanned,
};
use crate::compiler::ast::constructor::bit_pattern_section::SASTBitPatternSection;
use crate::compiler::ast::constructor::disassembly_section::SASTDisassemblySection;
use crate::compiler::ast::constructor::display_section::SASTDisplaySection;
use crate::compiler::ast::constructor::SASTConstructor;
use crate::compiler::ast::constructor::semantic_section::SASTSemanticSection;
use crate::compiler::ast::item::SASTItem;
use crate::compiler::CompilerInput;
use crate::compiler::error::SleighCompilerError;
use crate::compiler::lexer::{
    InputSpan,
    ParserInput,
    SleighTokens,
};
use crate::compiler::tokens::{
    SleighControlCharacter,
    SleighKeyword,
    SleighOperation,
    SleighToken,
};

#[derive(Debug, Default)]
pub struct SleighParser {}

impl SleighParser {
    pub fn parse<'src>(
        &self,
        source: &'src CompilerInput,
        tokens: SleighTokens<'src>,
    ) -> Result<SleighAST<'src>, SleighCompilerError> {
        let parsed = parser().parse(tokens.spanned());
        let mut diagnostic = vec![];
        trace!("Input: {:?}", tokens);
        trace!("Output: {:?}", parsed.output());
        parsed.errors().for_each(|err| {
            let error_span = err.span();
            let report = Report::build(ReportKind::Error, error_span.context(), error_span.start())
                .with_message("Parser Error")
                .with_label(Label::new(error_span.clone()).with_message(err.reason().to_string()))
                .finish();
            report
                .write((error_span.context(), Source::from(source.data())), &mut diagnostic)
                .unwrap();
        });
        if diagnostic.is_empty() {
            Ok(parsed.into_output().unwrap())
        } else {
            Err(SleighCompilerError::LexerError(String::from_utf8(diagnostic).unwrap()))
        }
    }
}


fn sleigh_whitespace<'tokens, 'src: 'tokens>() -> impl Parser<
    'tokens,
    ParserInput<'tokens, 'src>,
    (),
    extra::Err<Rich<'tokens, SleighToken<'src>, InputSpan<'src>>>,
> {
    just(SleighToken::Whitespace).repeated()
}

fn parser<'tokens, 'src: 'tokens>() -> impl Parser<
    'tokens,
    ParserInput<'tokens, 'src>,
    SleighAST<'src>,
    extra::Err<Rich<'tokens, SleighToken<'src>, InputSpan<'src>>>,
> {
    constructor().padded_by(sleigh_whitespace())
        .map(SASTItem::Constructor)
        .recover_with(skip_then_retry_until(any().ignored(), end()))
        .repeated()
        .collect::<Vec<_>>()
        .map(|items| SleighAST { items })
}

fn constructor<'tokens, 'src: 'tokens>() -> impl Parser<
    'tokens,
    ParserInput<'tokens, 'src>,
    Spanned<'src, SASTConstructor<'src>>,
    extra::Err<Rich<'tokens, SleighToken<'src>, InputSpan<'src>>>,
> {
    // The table can be either an identifier or it may be omitted, in which case the table referenced
    // will be the root table.
    let table = ident()
        .or_not()
        .map_with(|ident, extra| (ident, extra.span()));

    let display_section = display_section();
    let bit_pattern_section = bit_pattern_section();
    let disassembly_section = disassembly_section();
    let semantic_section = semantic_section();

    table
        .then(display_section)
        .then(bit_pattern_section)
        .then(disassembly_section)
        .then(semantic_section)
        .map_with(
            |((((table, display_section), bit_pattern_section), disassembly_section), semantic_section), extra| {
                (
                    SASTConstructor {
                        table,
                        display_section,
                        bit_pattern_section,
                        disassembly_section,
                        semantic_section,
                    },
                    extra.span(),
                )
            },
        )
}

/// Parse an ident token and return the contained string reference
fn ident<'tokens, 'src: 'tokens>() -> impl Parser<
    'tokens,
    ParserInput<'tokens, 'src>,
    &'src str,
    extra::Err<Rich<'tokens, SleighToken<'src>, InputSpan<'src>>>,
> + Copy {
    select! { SleighToken::Ident(ident) => ident }.labelled("identifier")
}

fn display_section<'tokens, 'src: 'tokens>() -> impl Parser<
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

    let identifiers = ident()
        .padded_by(
            any()
                .and_is(ident().not())
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

fn bit_pattern_section<'tokens, 'src: 'tokens>() -> impl Parser<
    'tokens,
    ParserInput<'tokens, 'src>,
    Spanned<'src, SASTBitPatternSection<'src>>,
    extra::Err<Rich<'tokens, SleighToken<'src>, InputSpan<'src>>>,
> {
    let end_of_bit_pattern = one_of([
        SleighToken::Control(SleighControlCharacter::OpenBracket),
        SleighToken::Control(SleighControlCharacter::OpenBrace),
    ]);

    any()
        .and_is(end_of_bit_pattern.not())
        .repeated()
        .map_with(|_, extra| {
            (
                SASTBitPatternSection {
                    constraints: ("", extra.span()),
                },
                extra.span(),
            )
        })
}

fn disassembly_section<'tokens, 'src: 'tokens>() -> impl Parser<
    'tokens,
    ParserInput<'tokens, 'src>,
    Spanned<'src, SASTDisassemblySection<'src>>,
    extra::Err<Rich<'tokens, SleighToken<'src>, InputSpan<'src>>>,
> {
    let open_bracket = just(SleighToken::Control(SleighControlCharacter::OpenBracket));
    let close_bracket = just(SleighToken::Control(SleighControlCharacter::CloseBracket));
    open_bracket
        .ignore_then(any().and_is(close_bracket.not()).repeated())
        .then_ignore(close_bracket)
        .or_not()
        .map_with(|actions, extra| {
            (
                SASTDisassemblySection {
                    actions: ("", extra.span()),
                },
                extra.span(),
            )
        })
}

fn semantic_section<'tokens, 'src: 'tokens>() -> impl Parser<
    'tokens,
    ParserInput<'tokens, 'src>,
    Spanned<'src, SASTSemanticSection<'src>>,
    extra::Err<Rich<'tokens, SleighToken<'src>, InputSpan<'src>>>,
> {
        let open_brace = just(SleighToken::Control(SleighControlCharacter::OpenBrace));
    let close_brace = just(SleighToken::Control(SleighControlCharacter::CloseBrace));
    open_brace
        .ignore_then(any().and_is(close_brace.not()).repeated())
        .then_ignore(close_brace)
        .map_with(|_, extra| {
            (
                SASTSemanticSection {
                    actions: ("", extra.span()),
                },
                extra.span(),
            )
        })
}
