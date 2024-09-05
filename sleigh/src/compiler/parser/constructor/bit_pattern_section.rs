use crate::compiler::ast::constructor::bit_pattern_section::{
    SASTBinaryOp,
    SASTBitPatternConstraint,
    SASTBitPatternSection,
    SASTConstraintOp,
    SASTConstraintPattern,
    SASTUnaryOp,
};
use crate::compiler::ast::Spanned;
use crate::compiler::lexer::ParserInput;
use crate::compiler::parser::helpers::spanned;
use crate::compiler::parser::{
    maybe_parenthesized,
    parenthesized,
    sast_ident,
    sast_ident_spanned,
    sleigh_control,
    sleigh_number,
    sleigh_operation,
    whitespace_padded,
    SleighParserExtra,
};
use crate::compiler::tokens::SleighControlCharacter::{
    OpenBrace,
    OpenBracket,
};
use crate::compiler::tokens::{
    SleighControlCharacter,
    SleighOperation,
    SleighToken,
};
use chumsky::input::MapExtra;
use chumsky::pratt::{
    infix,
    left,
    postfix,
    prefix,
};
use chumsky::prelude::{
    none_of,
    Recursive,
};
use chumsky::primitive::choice;
use chumsky::recovery::via_parser;
use chumsky::{
    select,
    Parser,
};

/// Parses the bit pattern section of a SLEIGH constructor.
/// See section 7.4 of the SLEIGH specification
pub fn bit_pattern_section<'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTBitPatternSection>, SleighParserExtra<'tokens>> {
    let mut constraint = Recursive::declare();

    let ident = whitespace_padded(sast_ident()).map_with(spanned);

    let bit_pattern = sleigh_number()
        .map_with(spanned)
        .map(SASTConstraintPattern::BitPattern);

    let constraint_pattern = whitespace_padded(
        sast_ident_spanned()
            .map(SASTConstraintPattern::Ident)
            .or(bit_pattern)
            .map_with(spanned),
    );

    let constraint_op = select! {
        SleighToken::Operation(SleighOperation::Equals) => SASTConstraintOp::Equals,
        SleighToken::Operation(SleighOperation::NotEquals) => SASTConstraintOp::NotEquals,
        SleighToken::Operation(SleighOperation::LessThan) => SASTConstraintOp::LessThan,
        SleighToken::Operation(SleighOperation::GreaterThan) => SASTConstraintOp::GreaterThan,
    }
    .map_with(spanned);

    let advanced_constraint = ident
        .clone()
        .then(constraint_op)
        .then(constraint_pattern)
        .map(|((lhs, op), rhs)| SASTBitPatternConstraint::AdvancedConstraint(lhs, op, rhs))
        .map_with(spanned)
        .labelled("constraint")
        .boxed();

    // An atom that can be unambiguously parsed. In this case either an identifier,
    // or a parenthesized constraint.
    let atom = whitespace_padded(maybe_parenthesized(advanced_constraint)
        .or(maybe_parenthesized(ident)
            .map(SASTBitPatternConstraint::Ident)
            .map_with(spanned))
        .or(parenthesized(constraint.clone())))
        .boxed();

    let operation = atom.clone().pratt((
        prefix(16, unary_operator(), fold_unary_prefix),
        postfix(15, unary_operator(), fold_unary_postfix),
        infix(left(10), binary_operator(SASTBinaryOp::And), fold_binary_operation),
        infix(left(9), binary_operator(SASTBinaryOp::Or), fold_binary_operation),
        infix(left(8), binary_operator(SASTBinaryOp::Concatenate), fold_binary_operation),
    ));

    // TODO: When parsing fails on a non-atom, the error is currently not displayed correctly.
    constraint.define(choice((operation, atom)));

    // Try to recover to the next section if parsing the bit pattern fails
    let recovery = none_of([SleighToken::Control(OpenBracket), SleighToken::Control(OpenBrace)])
        .repeated()
        .map(|_| SASTBitPatternConstraint::Invalid)
        .map_with(spanned);

    constraint
        .recover_with(via_parser(recovery))
        .map(SASTBitPatternSection::new)
        .map_with(spanned)
}

fn unary_operator<'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SleighControlCharacter>, SleighParserExtra<'tokens>> + Clone {
    whitespace_padded(
        sleigh_control(SleighControlCharacter::Ellipsis)
            .to(SleighControlCharacter::Ellipsis)
            .map_with(spanned),
    )
}

fn fold_unary_prefix<'tokens>(
    op: Spanned<SleighControlCharacter>,
    operand: Spanned<SASTBitPatternConstraint>,
    extra: &mut MapExtra<'tokens, '_, ParserInput<'tokens>, SleighParserExtra<'tokens>>,
) -> Spanned<SASTBitPatternConstraint> {
    spanned(
        SASTBitPatternConstraint::UnaryOp(
            Spanned::new(SASTUnaryOp::JustifyRight, op.span().clone()),
            Box::new(operand),
        ),
        extra,
    )
}

fn fold_unary_postfix<'tokens>(
    operand: Spanned<SASTBitPatternConstraint>,
    op: Spanned<SleighControlCharacter>,
    extra: &mut MapExtra<'tokens, '_, ParserInput<'tokens>, SleighParserExtra<'tokens>>,
) -> Spanned<SASTBitPatternConstraint> {
    spanned(
        SASTBitPatternConstraint::UnaryOp(Spanned::new(SASTUnaryOp::JustifyLeft, op.span().clone()), Box::new(operand)),
        extra,
    )
}

fn binary_operator<'tokens>(
    op: SASTBinaryOp
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTBinaryOp>, SleighParserExtra<'tokens>> + Clone {
    whitespace_padded(match op {
        SASTBinaryOp::And => sleigh_operation(SleighOperation::And)
            .to(SASTBinaryOp::And)
            .map_with(spanned)
            .boxed(),
        SASTBinaryOp::Or => sleigh_operation(SleighOperation::Or)
            .to(SASTBinaryOp::Or)
            .map_with(spanned)
            .boxed(),
        SASTBinaryOp::Concatenate => sleigh_control(SleighControlCharacter::SemiColon)
            .to(SASTBinaryOp::Concatenate)
            .map_with(spanned)
            .boxed(),
    })
}

fn fold_binary_operation<'tokens>(
    lhs: Spanned<SASTBitPatternConstraint>,
    op: Spanned<SASTBinaryOp>,
    rhs: Spanned<SASTBitPatternConstraint>,
    extra: &mut MapExtra<'tokens, '_, ParserInput<'tokens>, SleighParserExtra<'tokens>>,
) -> Spanned<SASTBitPatternConstraint> {
    spanned(SASTBitPatternConstraint::BinaryOp(Box::new(lhs), op, Box::new(rhs)), extra)
}
