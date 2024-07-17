use chumsky::error::Rich;
use chumsky::prelude::{
    just,
    Recursive,
};
use chumsky::{
    extra,
    select,
    Parser,
};

use crate::compiler::ast::constructor::bit_pattern_section::{
    SASTBinaryOp,
    SASTBitPatternConstraint,
    SASTBitPatternSection,
    SASTConstraintOp,
    SASTConstraintPattern,
    SASTUnaryOp,
};
use crate::compiler::ast::Spanned;
use crate::compiler::lexer::{
    InputSpan,
    ParserInput,
};
use crate::compiler::parser::{
    maybe_parenthesized,
    parenthesized,
    sast_ident,
    sleigh_inline_whitespace,
    sleigh_number,
    sleigh_whitespace,
    SleighParserExtra,
};
use crate::compiler::tokens::{
    SleighControlCharacter,
    SleighOperation,
    SleighToken,
};

/// Parses the bit pattern section of a SLEIGH constructor.
/// See section 7.4 of the SLEIGH specification
pub(super) fn bit_pattern_section<'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTBitPatternSection>, SleighParserExtra<'tokens>> {
    let mut constraint = Recursive::declare();

    let op = select! {
        SleighToken::Operation(SleighOperation::And) => SASTBinaryOp::And,
        SleighToken::Operation(SleighOperation::Or) => SASTBinaryOp::Or,
        SleighToken::Control(SleighControlCharacter::SemiColon) => SASTBinaryOp::Concatenate,
    }
    .map_with(|op, extra| (op, extra.span()))
    .labelled("operator");

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
        .map_with(|((lhs, op), rhs), extra| (SASTBitPatternConstraint::AdvancedConstraint(lhs, op, rhs), extra.span()))
        .labelled("constraint");

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
    }
    .labelled("unary operation");

    let atom = unary_op.or(atom);

    let binary_op = atom
        .clone()
        .foldl_with(op.then(atom.clone()).repeated(), |lhs, (op, rhs), extra| {
            (SASTBitPatternConstraint::BinaryOp(Box::new(lhs), op, Box::new(rhs)), extra.span())
        })
        .labelled("binary operation");

    constraint.define(binary_op);

    constraint.map_with(|constraint, extra| (SASTBitPatternSection { constraint }, extra.span()))
}
