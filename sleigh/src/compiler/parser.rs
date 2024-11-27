use crate::compiler::ast::common::SASTIdent;
use crate::compiler::ast::item::SASTItem;
use crate::compiler::ast::{
    SleighAST,
    Spanned,
};
use crate::compiler::error::{
    InternalError,
    InternalErrorType,
    SleighCompilerError,
};
use crate::compiler::inputcache::CompilerInputCache;
use crate::compiler::lexer::{
    InputSpan,
    ParserInput,
    SleighTokens,
};
use crate::compiler::parser::attach::attach;
use crate::compiler::parser::constructor::constructor;
use crate::compiler::parser::define::define;
use crate::compiler::parser::helpers::spanned;
use crate::compiler::parser::included::included_tokens;
use crate::compiler::parser::pcode_macro::pcode_macro;
use crate::compiler::parser::with::with_block;
use crate::compiler::tokens::{
    SleighControlCharacter,
    SleighKeyword,
    SleighOperation,
    SleighToken,
};
use crate::compiler::{
    CompilerContext,
    SleighCompilerResult,
};
use crate::spec::builder::{
    ProcessorSpecificationBuilder,
    StringId,
};
use ariadne::{
    Label,
    Report,
    ReportKind,
};
use chumsky::combinator::{
    DelimitedBy,
    PaddedBy,
};
use chumsky::error::Rich;
use chumsky::extra::ParserExtra;
use chumsky::input::MapExtra;
use chumsky::label::LabelError;
use chumsky::prelude::{
    Recursive,
    any,
    end,
    skip_then_retry_until,
    via_parser,
};
use chumsky::primitive::{
    choice,
    just,
};
use chumsky::span::Span;
use chumsky::{
    IterParser,
    ParseResult,
    Parser,
    extra,
    select,
};
use std::fmt::Display;

mod attach;
mod constructor;
mod define;
mod helpers;
mod included;
mod pcode;
mod pcode_macro;
#[cfg(test)]
mod test;
mod with;

#[derive(Debug, Default)]
pub struct SleighParser {}

impl SleighParser {
    pub fn parse(
        &self,
        input_cache: &mut CompilerInputCache,
        context: &mut ProcessorSpecificationBuilder,
        tokens: SleighTokens,
    ) -> SleighCompilerResult<SleighAST> {
        let parsed = parser().parse_with_state(tokens.spanned(), context);
        report_errors(input_cache, parsed)
    }
}

pub fn report_errors<T>(
    mut input_cache: &mut CompilerInputCache,
    parsed: ParseResult<T, Rich<SleighToken, InputSpan>>,
) -> SleighCompilerResult<T> {
    let mut diagnostic = vec![];
    parsed.errors().for_each(|err| {
        let error_span = err.span();
        let report = Report::build(ReportKind::Error, error_span.clone())
            .with_message("Parser Error")
            .with_label(Label::new(error_span.clone()).with_message(err.reason().to_string()))
            .finish();
        report
            .write(&mut input_cache, &mut diagnostic)
            .expect("There is no case in which this may fail.");
    });
    if diagnostic.is_empty() && parsed.has_output() {
        Ok(parsed.into_output().expect("We mus have output here."))
    } else {
        diagnostic.extend(
            format!(
                "───────────────────────────────\nFound a total of {} errors.\n───────────────────────────────",
                parsed.errors().len()
            )
            .as_bytes(),
        );
        Err(
            SleighCompilerError::ParserError(String::from_utf8(diagnostic).expect("We only write utf8 strings."))
                .into(),
        )
    }
}

type SleighParserError<'tokens> = Rich<'tokens, SleighToken, InputSpan>;
type SleighParserExtra<'tokens> = extra::Full<SleighParserError<'tokens>, ProcessorSpecificationBuilder, ()>;

fn parser<'tokens, 'src: 'tokens>() -> impl Parser<'tokens, ParserInput<'tokens>, SleighAST, SleighParserExtra<'tokens>>
{
    items_parser().map(|items| SleighAST { items })
}

fn items_parser<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, ParserInput<'tokens>, Vec<SASTItem>, SleighParserExtra<'tokens>> + Clone {
    let mut items = Recursive::declare();

    let attach = attach().map(SASTItem::Attach);
    let constructor = constructor().map(SASTItem::Constructor);
    let define = define().map(SASTItem::Define);
    let pcode_macro = pcode_macro().map(SASTItem::Macro);
    let with = with_block(items.clone()).map(SASTItem::With);

    let items_section = choice((attach, constructor, pcode_macro, define, with))
        .padded_by(sleigh_whitespace())
        .recover_with(skip_then_retry_until(
            any()
                .and_is(select! {SleighToken::IncludedTokens(_) => () }.not())
                .ignored(),
            end(),
        ))
        .repeated()
        .at_least(1)
        .collect::<Vec<_>>()
        .boxed();

    items.define(
        items_section
            .or(included_tokens(items.clone()).boxed())
            .or(one_sleigh_whitespace()
                .repeated()
                .at_least(1)
                .ignore_then(end().map(|_| Vec::new())))
            .repeated()
            .collect::<Vec<_>>()
            .map(|items| items.into_iter().flatten().collect()),
    );
    items.boxed()
}

pub fn sleigh_inline_whitespace<'tokens, 'src, E>() -> impl Parser<'tokens, ParserInput<'tokens>, (), E> + Clone
where
    'src: 'tokens,
    E: ParserExtra<'tokens, ParserInput<'tokens>>,
{
    just(SleighToken::Whitespace).ignored().repeated()
}

fn one_sleigh_whitespace<'tokens, E>() -> impl Parser<'tokens, ParserInput<'tokens>, (), E> + Clone
where
    E: ParserExtra<'tokens, ParserInput<'tokens>>,
{
    // Comments are treated as whitespace
    just(SleighToken::Whitespace)
        .ignored()
        .or(just(SleighToken::Newline).ignored())
        .or(select! { SleighToken::Comment(_) => () })
}

fn sleigh_whitespace<'tokens, E>() -> impl Parser<'tokens, ParserInput<'tokens>, (), E> + Clone
where
    E: ParserExtra<'tokens, ParserInput<'tokens>>,
{
    one_sleigh_whitespace().repeated()
}

fn at_least_one_sleigh_whitespace<'tokens, E>() -> impl Parser<'tokens, ParserInput<'tokens>, (), E> + Clone
where
    E: ParserExtra<'tokens, ParserInput<'tokens>>,
{
    one_sleigh_whitespace().repeated().at_least(1)
}

/// Parse an ident token and return the contained string reference
fn sleigh_ident<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, ParserInput<'tokens>, StringId, SleighParserExtra<'tokens>> + Clone {
    let ident = select! { SleighToken::Ident(ident) => ident };
    ident.map_err(|mut e: SleighParserError| {
        <SleighParserError as LabelError<'_, ParserInput<'_>, _>>::label_with(&mut e, "Identifier");
        e
    })
}

/// Parse an ident token and return a SASTIdent tree node
fn sast_ident<'tokens>() -> impl Parser<'tokens, ParserInput<'tokens>, SASTIdent, SleighParserExtra<'tokens>> + Clone {
    sleigh_ident().map(SASTIdent)
}

fn sast_ident_spanned<'tokens>()
-> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTIdent>, SleighParserExtra<'tokens>> + Clone {
    sleigh_ident()
        .map(SASTIdent)
        .map_with(spanned)
        .labelled("identifier")
}

fn sleigh_number<'tokens, T>() -> impl Parser<'tokens, ParserInput<'tokens>, T, SleighParserExtra<'tokens>> + Clone
where
    T: TryFrom<u64>,
    <T as TryFrom<u64>>::Error: Display,
{
    let parsed_number = select! {
        SleighToken::Decimal(str_id) => (str_id, 10),
        SleighToken::Hexadecimal(str_id) => (str_id, 16),
        SleighToken::Binary(str_id) => (str_id, 2),
    };
    parsed_number
        .map_with(|(num, radix), extra: &mut MapExtra<'_, '_, _, SleighParserExtra<'tokens>>| {
            u64::from_str_radix(
                extra
                    .state()
                    .string(num)
                    .expect("The string must be interned"),
                radix,
            )
        })
        .try_map(|num, span| num.map_err(|e| Rich::custom(span, e)))
        .try_map_with(|num, extra| T::try_from(num).map_err(|e| Rich::custom(extra.span(), e)))
        .recover_with(via_parser(
            parsed_number.try_map_with(|_, extra| T::try_from(0).map_err(|e| Rich::custom(extra.span(), e))),
        ))
        .boxed()
}

/// Parses the same thing as `P`, but only if it is surrounded by parentheses.
pub fn parenthesized<'tokens, 'src, P, O, E>(
    parser: P
) -> DelimitedBy<
    P,
    impl Parser<'tokens, ParserInput<'tokens>, (), E> + Clone,
    impl Parser<'tokens, ParserInput<'tokens>, (), E> + Clone,
    (),
    (),
>
where
    'src: 'tokens,
    P: Parser<'tokens, ParserInput<'tokens>, O, E>,
    E: ParserExtra<'tokens, ParserInput<'tokens>>,
{
    parser.delimited_by(
        just(SleighToken::Control(SleighControlCharacter::OpenParen)).ignored(),
        just(SleighToken::Control(SleighControlCharacter::CloseParen)).ignored(),
    )
}

/// Parses the same thing as the provided parser `P`, but also succeeds if the element to parse
/// is surrounded by parentheses.
pub fn maybe_parenthesized<'tokens, 'src, P, O, E>(
    parser: P
) -> impl Parser<'tokens, ParserInput<'tokens>, O, E> + Clone
where
    'src: 'tokens,
    P: Parser<'tokens, ParserInput<'tokens>, O, E> + Clone,
    E: ParserExtra<'tokens, ParserInput<'tokens>>,
{
    parser.clone().or(parenthesized(parser))
}

/// Parses the same thing as the supplied parser, but also consumes surrounding whitespace excluding newline tokens.
pub fn inline_whitespace_padded<'tokens, 'src, P, O, E>(
    parser: P
) -> PaddedBy<P, impl Parser<'tokens, ParserInput<'tokens>, (), E> + Clone, ()>
where
    'src: 'tokens,
    P: Parser<'tokens, ParserInput<'tokens>, O, E> + Clone,
    E: ParserExtra<'tokens, ParserInput<'tokens>>,
{
    parser.padded_by(sleigh_inline_whitespace())
}

/// Parses the same thing as the supplied parser, but also consumes surrounding whitespace including newline tokens.
pub fn whitespace_padded<'tokens, 'src, P, O, E>(
    parser: P
) -> PaddedBy<P, impl Parser<'tokens, ParserInput<'tokens>, (), E> + Clone, ()>
where
    'src: 'tokens,
    P: Parser<'tokens, ParserInput<'tokens>, O, E> + Clone,
    E: ParserExtra<'tokens, ParserInput<'tokens>>,
{
    parser.padded_by(sleigh_whitespace())
}

/// Parses a sleigh keyword. The output is always ().
fn sleigh_keyword<'tokens, 'src: 'tokens>(
    keyword: SleighKeyword
) -> impl Parser<'tokens, ParserInput<'tokens>, (), SleighParserExtra<'tokens>> + Clone {
    just(SleighToken::Keyword(keyword)).ignored()
}

/// Parses a sleigh control character. The output is always ().
fn sleigh_control<'tokens, 'src: 'tokens>(
    control_character: SleighControlCharacter
) -> impl Parser<'tokens, ParserInput<'tokens>, (), SleighParserExtra<'tokens>> + Clone {
    just(SleighToken::Control(control_character)).ignored()
}

/// Parses a sleigh operation. The output is always ().
fn sleigh_operation<'tokens, 'src: 'tokens>(
    operation: SleighOperation
) -> impl Parser<'tokens, ParserInput<'tokens>, (), SleighParserExtra<'tokens>> + Clone {
    just(SleighToken::Operation(operation)).ignored()
}
