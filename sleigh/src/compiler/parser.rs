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
use chumsky::extra::ParserExtra;
use chumsky::input::Input;
use chumsky::prelude::{
    any,
    end,
    skip_then_retry_until,
    skip_until,
    via_parser,
};
use chumsky::primitive::just;
use chumsky::recursive::Recursive;
use chumsky::span::Span;
use tracing::trace;

use crate::compiler::ast::{
    SleighAST,
    Spanned,
};
use crate::compiler::ast::common::SASTIdent;
use crate::compiler::ast::constructor::bit_pattern_section::{
    SASTBinaryOp,
    SASTBitPatternConstraint,
    SASTBitPatternSection,
    SASTConstraintOp,
    SASTConstraintPattern,
    SASTUnaryOp,
};
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
use crate::compiler::parser::constructor::constructor;
use crate::compiler::parser::pcode_macro::pcode_macro;
use crate::compiler::tokens::{
    SleighControlCharacter,
    SleighKeyword,
    SleighOperation,
    SleighToken,
};

mod constructor;
mod pcode;
mod pcode_macro;

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
        // trace!("Input: {:?}", tokens);
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

fn sleigh_whitespace<'tokens, 'src, E>(
) -> impl Parser<'tokens, ParserInput<'tokens, 'src>, (), E> + Copy
where
    'src: 'tokens,
    E: ParserExtra<'tokens, ParserInput<'tokens, 'src>>,
{
    // Comments are treated as whitespace
    just(SleighToken::Whitespace)
        .ignored()
        .or(select! { SleighToken::Comment(_) }.ignored())
        .repeated()
}

fn parser<'tokens, 'src: 'tokens>() -> impl Parser<
    'tokens,
    ParserInput<'tokens, 'src>,
    SleighAST<'src>,
    extra::Err<Rich<'tokens, SleighToken<'src>, InputSpan<'src>>>,
> {
    let constructor = constructor().map(SASTItem::Constructor);
    let pcode_macro = pcode_macro().map(SASTItem::Macro);
    constructor
        .or(pcode_macro)
        .padded_by(sleigh_whitespace())
        .recover_with(skip_then_retry_until(any().ignored(), end()))
        .repeated()
        .collect::<Vec<_>>()
        .map(|items| SleighAST { items })
}

/// Parse an ident token and return the contained string reference
fn sleigh_ident<'tokens, 'src: 'tokens>() -> impl Parser<
    'tokens,
    ParserInput<'tokens, 'src>,
    &'src str,
    extra::Err<Rich<'tokens, SleighToken<'src>, InputSpan<'src>>>,
> + Copy {
    select! { SleighToken::Ident(ident) => ident }.labelled("identifier")
}

/// Parse an ident token and return a SASTIdent tree node
fn sast_ident<'tokens, 'src: 'tokens>() -> impl Parser<
    'tokens,
    ParserInput<'tokens, 'src>,
    SASTIdent<'src>,
    extra::Err<Rich<'tokens, SleighToken<'src>, InputSpan<'src>>>,
> + Copy {
    sleigh_ident().map(SASTIdent)
}

fn sleigh_number<'tokens, 'src: 'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens, 'src>, u64, extra::Err<Rich<'tokens, SleighToken<'src>, InputSpan<'src>>>>
       + Copy {
    let parsed_number = select! {
        SleighToken::Decimal(decimal) => u64::from_str_radix(decimal, 10),
        SleighToken::Hexadecimal(decimal) => u64::from_str_radix(decimal, 16),
        SleighToken::Binary(decimal) => u64::from_str_radix(decimal, 2),
    };
    parsed_number.try_map(|num, span| num.map_err(|e| Rich::custom(span, e)))
}

/// Parses the same thing as `P`, but only if it is surrounded by parentheses.
fn parenthesized<'tokens, 'src, P, O, E>(parser: P) -> impl Parser<'tokens, ParserInput<'tokens, 'src>, O, E> + Clone
where
    'src: 'tokens,
    P: Parser<'tokens, ParserInput<'tokens, 'src>, O, E> + Clone,
    E: ParserExtra<'tokens, ParserInput<'tokens, 'src>>,
{
    parser.delimited_by(
        just(SleighToken::Control(SleighControlCharacter::OpenParen)),
        just(SleighToken::Control(SleighControlCharacter::CloseParen)),
    )
}

/// Parses the same thing as the provided parser `P`, but also succeeds if the element to parse
/// is surrounded by parentheses.
fn maybe_parenthesized<'tokens, 'src, P, O, E>(
    parser: P
) -> impl Parser<'tokens, ParserInput<'tokens, 'src>, O, E> + Clone
where
    'src: 'tokens,
    P: Parser<'tokens, ParserInput<'tokens, 'src>, O, E> + Clone,
    E: ParserExtra<'tokens, ParserInput<'tokens, 'src>>,
{
    parser.clone().or(parenthesized(parser))
}

fn whitespace_padded<'tokens, 'src, P, O, E>(
    parser: P
) -> impl Parser<'tokens, ParserInput<'tokens, 'src>, O, E> + Clone
where
    'src: 'tokens,
    P: Parser<'tokens, ParserInput<'tokens, 'src>, O, E> + Clone,
    E: ParserExtra<'tokens, ParserInput<'tokens, 'src>>,
{
    parser.padded_by(sleigh_whitespace())
}
