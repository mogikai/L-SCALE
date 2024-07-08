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
use chumsky::input::{
    Input,
    MapExtra,
};
use chumsky::prelude::{
    any,
    choice,
    end,
    one_of,
    skip_then_retry_until,
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

fn sleigh_whitespace<'tokens, 'src: 'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens, 'src>, (), extra::Err<Rich<'tokens, SleighToken<'src>, InputSpan<'src>>>> + Copy
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
    constructor()
        .padded_by(sleigh_whitespace())
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
    let table = sleigh_ident()
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

/// Parses the bit pattern section of a SLEIGH constructor.
/// See section 7.4 of the SLEIGH specification
fn bit_pattern_section<'tokens, 'src: 'tokens>() -> impl Parser<
    'tokens,
    ParserInput<'tokens, 'src>,
    Spanned<'src, SASTBitPatternSection<'src>>,
    extra::Err<Rich<'tokens, SleighToken<'src>, InputSpan<'src>>>,
> {
    let mut constraint = Recursive::declare();

    let op = select! {
        SleighToken::Operation(SleighOperation::And) => SASTBinaryOp::And,
        SleighToken::Operation(SleighOperation::Or) => SASTBinaryOp::Or,
        SleighToken::Control(SleighControlCharacter::SemiColon) => SASTBinaryOp::Concatenate,
    }
    .map_with(|op, extra| (op, extra.span()));

    let ident = sast_ident()
        .padded_by(sleigh_whitespace())
        .map_with(|item, extra| (item, extra.span()));

    let constraint_op = select! {
        SleighToken::Operation(SleighOperation::Equals) => SASTConstraintOp::Equals,
        SleighToken::Operation(SleighOperation::NotEquals) => SASTConstraintOp::NotEquals,
        SleighToken::Operation(SleighOperation::GreaterThan) => SASTConstraintOp::GreaterThan,
        SleighToken::Operation(SleighOperation::LessThan) => SASTConstraintOp::LessThan,
    }
    .map_with(|op, extra| (op, extra.span()))
    .padded_by(sleigh_whitespace());

    let bit_pattern = sleigh_number().map(SASTConstraintPattern::BitPattern);

    let constraint_pattern = sast_ident()
        .map(SASTConstraintPattern::Ident)
        .or(bit_pattern)
        .map_with(|pattern, extra| (pattern, extra.span()))
        .padded_by(sleigh_whitespace());

    let advanced_constraint = ident
        .then(constraint_op)
        .then(constraint_pattern)
        .map_with(|((lhs, op), rhs), extra| (SASTBitPatternConstraint::AdvancedConstraint(lhs, op, rhs), extra.span()));

    // An atom that can be unambiguously parsed. In this case either an identifier,
    // or a parenthesized constraint.
    let atom = maybe_parenthesized(advanced_constraint)
        .or(maybe_parenthesized(ident)
            .map(SASTBitPatternConstraint::Ident)
            .map_with(|item, extra| (item, extra.span())))
        .or(parenthesized(constraint.clone()))
        .padded_by(sleigh_whitespace());

    // A unary op is currently only justify left or justify right.
    let unary_op = {
        let ellipsis = just(SleighToken::Control(SleighControlCharacter::Ellipsis));
        // Justify left ist represented by an ellipsis, followed by an expression, and
        let justify_left = ellipsis
            .to_span()
            .then(atom.clone())
            .map_with(|(justify, atom), extra| {
                (SASTBitPatternConstraint::UnaryOp((SASTUnaryOp::JustifyLeft, justify), Box::new(atom)), extra.span())
            });
        // Justify right is represented by an expression, followed by an ellipsis.
        let justify_right = atom
            .clone()
            .then(ellipsis.to_span())
            .map_with(|(atom, justify), extra| {
                (
                    SASTBitPatternConstraint::UnaryOp((SASTUnaryOp::JustifyRight, justify), Box::new(atom)),
                    extra.span(),
                )
            });

        justify_left
            .or(justify_right)
            .padded_by(sleigh_whitespace())
    };

    let atom = unary_op.or(atom);

    let binary_op = atom
        .clone()
        .foldl_with(op.then(atom.clone()).repeated(), |lhs, (op, rhs), extra| {
            (SASTBitPatternConstraint::BinaryOp(Box::new(lhs), op, Box::new(rhs)), extra.span())
        });

    constraint.define(binary_op);

    constraint.map_with(|constraint, extra| (SASTBitPatternSection { constraint }, extra.span()))
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
