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
    sleigh_control,
    sleigh_number,
    sleigh_whitespace,
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
use chumsky::prelude::{
    none_of,
    Recursive,
};
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

    let op = select! {
        SleighToken::Operation(SleighOperation::And) => SASTBinaryOp::And,
        SleighToken::Operation(SleighOperation::Or) => SASTBinaryOp::Or,
        SleighToken::Control(SleighControlCharacter::SemiColon) => SASTBinaryOp::Concatenate,
    }
    .map_with(spanned)
    .labelled("operator");

    let ident = whitespace_padded(sast_ident()).map_with(spanned);

    let constraint_op = whitespace_padded(
        select! {
            SleighToken::Operation(SleighOperation::Equals) => SASTConstraintOp::Equals,
            SleighToken::Operation(SleighOperation::NotEquals) => SASTConstraintOp::NotEquals,
            SleighToken::Operation(SleighOperation::GreaterThan) => SASTConstraintOp::GreaterThan,
            SleighToken::Operation(SleighOperation::LessThan) => SASTConstraintOp::LessThan,
        }
        .map_with(spanned),
    );

    let bit_pattern = sleigh_number().map(SASTConstraintPattern::BitPattern);

    let constraint_pattern = whitespace_padded(
        sast_ident()
            .map(SASTConstraintPattern::Ident)
            .or(bit_pattern)
            .map_with(spanned),
    );

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
    let atom = maybe_parenthesized(advanced_constraint)
        .or(maybe_parenthesized(ident)
            .map(SASTBitPatternConstraint::Ident)
            .map_with(spanned))
        .or(parenthesized(constraint.clone()))
        .boxed();

    // A unary op is currently only justify left or justify right.
    let unary_op = {
        let ellipsis = whitespace_padded(sleigh_control(SleighControlCharacter::Ellipsis));
        // Justify left ist represented by an ellipsis, followed by an expression, and
        let justify_left = ellipsis
            .clone()
            .to(SASTUnaryOp::JustifyLeft)
            .map_with(spanned)
            .then(atom.clone().map(Box::new))
            .map(|(justify, atom)| SASTBitPatternConstraint::UnaryOp(justify, atom))
            .map_with(spanned)
            .boxed();
        // Justify right is represented by an expression, followed by an ellipsis.
        let justify_right = atom
            .clone()
            .map(Box::new)
            .then(ellipsis.to(SASTUnaryOp::JustifyRight).map_with(spanned))
            .map(|(atom, justify)| SASTBitPatternConstraint::UnaryOp(justify, atom))
            .map_with(spanned)
            .boxed();

        justify_left.or(justify_right)
    }
    .labelled("unary operation");

    let atom = whitespace_padded(unary_op.or(atom));

    // TODO: When parsing fails on a non-atom, the error is currently not displayed correctly.
    let binary_op = atom
        .clone()
        .foldl_with(op.then(atom.clone()).repeated(), spanned_binary_op)
        .labelled("binary operation");

    constraint.define(binary_op);

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

fn spanned_binary_op<'tokens>(
    lhs: Spanned<SASTBitPatternConstraint>,
    (op, rhs): (Spanned<SASTBinaryOp>, Spanned<SASTBitPatternConstraint>),
    extra: &mut MapExtra<'tokens, '_, ParserInput<'tokens>, SleighParserExtra<'tokens>>,
) -> Spanned<SASTBitPatternConstraint> {
    spanned(SASTBitPatternConstraint::BinaryOp(Box::new(lhs), op, Box::new(rhs)), extra)
}
