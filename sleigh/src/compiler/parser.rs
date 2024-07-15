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
use chumsky::combinator::{
    DelimitedBy,
    PaddedBy,
};
use chumsky::error::Rich;
use chumsky::extra::ParserExtra;
use chumsky::input::{Input, MapExtra};
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

use crate::compiler::{CompilerContext, CompilerInput, CompilerInputCache, InputId, StringId};
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
        input_cache: &CompilerInputCache,
        context: &mut CompilerContext,
        input_id: InputId,
        tokens: SleighTokens,
    ) -> Result<SleighAST, SleighCompilerError> {
        let source = input_cache.input(input_id)?;
        let parsed = parser().parse_with_state(tokens.spanned(), context);
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

type SleighParserExtra<'tokens> = extra::Full<Rich<'tokens, SleighToken, InputSpan>, CompilerContext, ()>;

fn sleigh_whitespace<'tokens, 'src, E>() -> impl Parser<'tokens, ParserInput<'tokens>, (), E> + Copy
where
    'src: 'tokens,
    E: ParserExtra<'tokens, ParserInput<'tokens>>,
{
    // Comments are treated as whitespace
    just(SleighToken::Whitespace)
        .ignored()
        .or(select! { SleighToken::Comment(_) }.ignored())
        .repeated()
}

fn parser<'tokens, 'src: 'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, SleighAST, SleighParserExtra<'tokens>> {
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
fn sleigh_ident<'tokens, 'src: 'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, StringId, SleighParserExtra<'tokens>> + Copy {
    select! { SleighToken::Ident(ident) => ident }.labelled("identifier")
}

/// Parse an ident token and return a SASTIdent tree node
fn sast_ident<'tokens>() -> impl Parser<'tokens, ParserInput<'tokens>, SASTIdent, SleighParserExtra<'tokens>> + Copy {
    sleigh_ident().map(SASTIdent)
}

fn sast_ident_spanned<'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTIdent>, SleighParserExtra<'tokens>> + Copy {
    sleigh_ident().map_with(|ident, extra| (SASTIdent(ident), extra.span()))
}

fn sleigh_number<'tokens>() -> impl Parser<'tokens, ParserInput<'tokens>, u64, SleighParserExtra<'tokens>> + Copy {
    let parsed_number = select! {
        SleighToken::Decimal(str_id) => (str_id, 10),
        SleighToken::Hexadecimal(str_id) => (str_id, 16),
        SleighToken::Binary(str_id) => (str_id, 2),
    };
    parsed_number
        .map_with(|(num, radix), extra: &mut MapExtra<'_, '_, _, SleighParserExtra<'tokens>>| u64::from_str_radix(extra.state().string(num).expect("The string must be interned"), radix))
        .try_map(|num, span| num.map_err(|e| Rich::custom(span, e)))
}

/// Parses the same thing as `P`, but only if it is surrounded by parentheses.
fn parenthesized<'tokens, 'src, P, O, E>(
    parser: P
) -> DelimitedBy<
    P,
    impl Parser<'tokens, ParserInput<'tokens>, (), E> + Copy,
    impl Parser<'tokens, ParserInput<'tokens>, (), E> + Copy,
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
fn maybe_parenthesized<'tokens, 'src, P, O, E>(
    parser: P
) -> impl Parser<'tokens, ParserInput<'tokens>, O, E> + Clone
where
    'src: 'tokens,
    P: Parser<'tokens, ParserInput<'tokens>, O, E> + Clone,
    E: ParserExtra<'tokens, ParserInput<'tokens>>,
{
    parser.clone().or(parenthesized(parser))
}

fn whitespace_padded<'tokens, 'src, P, O, E>(
    parser: P
) -> PaddedBy<P, impl Parser<'tokens, ParserInput<'tokens>, (), E> + Copy, ()>
where
    'src: 'tokens,
    P: Parser<'tokens, ParserInput<'tokens>, O, E> + Clone,
    E: ParserExtra<'tokens, ParserInput<'tokens>>,
{
    parser.padded_by(sleigh_whitespace())
}

fn sleigh_keyword<'tokens, 'src: 'tokens>(
    keyword: SleighKeyword
) -> impl Parser<'tokens, ParserInput<'tokens>, (), SleighParserExtra<'tokens>> + Copy {
    just(SleighToken::Keyword(keyword)).ignored()
}

fn sleigh_control<'tokens, 'src: 'tokens>(
    control_character: SleighControlCharacter
) -> impl Parser<'tokens, ParserInput<'tokens>, (), SleighParserExtra<'tokens>> + Copy {
    just(SleighToken::Control(control_character)).ignored()
}

fn sleigh_operation<'tokens, 'src: 'tokens>(
    operation: SleighOperation
) -> impl Parser<'tokens, ParserInput<'tokens>, (), SleighParserExtra<'tokens>> + Copy {
    just(SleighToken::Operation(operation)).ignored()
}
