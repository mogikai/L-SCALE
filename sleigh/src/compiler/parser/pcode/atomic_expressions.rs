use std::fmt::{
    Debug,
    Display,
};

use chumsky::error::Rich;
use chumsky::prelude::{
    none_of,
    via_parser,
};
use chumsky::{
    IterParser,
    Parser,
};

use SleighOperation::{
    GreaterThan,
    LessThan,
};

use crate::compiler::ast::common::SASTIdent;
use crate::compiler::ast::pcode::addressof::SASTPCodeAddressOf;
use crate::compiler::ast::pcode::bitrange::SASTPCodeBitRange;
use crate::compiler::ast::pcode::deref::SASTPCodeDeref;
use crate::compiler::ast::pcode::expression::SASTPCodeExpression;
use crate::compiler::ast::pcode::invocation::SASTPCodeInvocation;
use crate::compiler::ast::pcode::subpiece::{
    SASTPCodeSubpiece,
    SASTPCodeSubpieceTarget,
};
use crate::compiler::ast::Spanned;
use crate::compiler::lexer::ParserInput;
use crate::compiler::parser::helpers::spanned;
use crate::compiler::parser::{
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
    CloseBracket,
    CloseParen,
    Colon,
    Comma,
    OpenBracket,
    OpenParen,
};
use crate::compiler::tokens::SleighOperation::{
    And,
    Star,
};
use crate::compiler::tokens::{
    SleighControlCharacter,
    SleighOperation,
    SleighToken,
};

pub fn pcode_ident<'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTIdent>, SleighParserExtra<'tokens>> + Clone {
    whitespace_padded(sast_ident_spanned())
}

/// Parse a number literal and convert it into T. Surrounding whitespace will be consumed.
pub fn pcode_number_literal<'tokens, T>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<T>, SleighParserExtra<'tokens>> + Clone
where
    T: TryFrom<u64> + Debug,
    <T as TryFrom<u64>>::Error: Display,
{
    whitespace_padded(
        sleigh_number()
            .try_map_with(|num, extra| T::try_from(num).map_err(|e| Rich::custom(extra.span(), e)))
            .map_with(spanned),
    )
}

pub fn pcode_subpiece<'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTPCodeSubpiece>, SleighParserExtra<'tokens>> + Clone {
    let size = pcode_number_literal();

    let target = pcode_ident()
        .map(SASTPCodeSubpieceTarget::Ident)
        .or(pcode_number_literal().map(SASTPCodeSubpieceTarget::Literal))
        .map_with(spanned);

    let colon_subpiece = target
        .clone()
        .then_ignore(sleigh_control(SleighControlCharacter::Colon))
        .then(size.clone());

    let paren_subpiece = target.then(parenthesized(size));

    colon_subpiece
        .or(paren_subpiece)
        .map(|(target, size)| SASTPCodeSubpiece { target, size })
        .map_with(spanned)
}

pub fn pcode_bitrange<'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTPCodeBitRange>, SleighParserExtra<'tokens>> + Clone {
    let lsb = pcode_number_literal();
    let bits = pcode_number_literal();
    pcode_ident()
        .then(
            lsb.then_ignore(sleigh_control(Comma))
                .then(bits)
                .delimited_by(sleigh_control(OpenBracket), sleigh_control(CloseBracket)),
        )
        .map(|(ident, (lsb, bits))| SASTPCodeBitRange { ident, lsb, bits })
        .map_with(spanned)
}

/// Parse a pcode label identified by `<some_label>`.
pub fn pcode_label<'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTIdent>, SleighParserExtra<'tokens>> + Clone {
    sleigh_operation(LessThan)
        .ignore_then(sast_ident())
        .then_ignore(sleigh_operation(GreaterThan))
        .map_with(spanned)
}

pub fn pcode_deref<'tokens, P>(
    expression_parser: P
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTPCodeDeref>, SleighParserExtra<'tokens>> + Clone
where
    P: Parser<'tokens, ParserInput<'tokens>, Spanned<SASTPCodeExpression>, SleighParserExtra<'tokens>> + Clone,
{
    let space_recovery = none_of(SleighToken::Control(CloseBracket))
        .repeated()
        .map(|_| None);
    let space = whitespace_padded(
        whitespace_padded(
            sast_ident_spanned()
                .map(Some)
                .recover_with(via_parser(space_recovery)),
        )
        .delimited_by(sleigh_control(OpenBracket), sleigh_control(CloseBracket)),
    );

    let size = whitespace_padded(sleigh_control(Colon)).ignore_then(pcode_number_literal());

    sleigh_operation(Star)
        .ignore_then(space.or_not().map(Option::flatten))
        .then(size.or_not())
        .then(whitespace_padded(expression_parser.map(Box::new)))
        .map(|((space, size), expression)| SASTPCodeDeref {
            expression,
            space,
            size,
        })
        .map_with(spanned)
}

pub fn pcode_address_of<'tokens, P>(
    expression_parser: P
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTPCodeAddressOf>, SleighParserExtra<'tokens>> + Clone
where
    P: Parser<'tokens, ParserInput<'tokens>, Spanned<SASTPCodeExpression>, SleighParserExtra<'tokens>> + Clone,
{
    let size = whitespace_padded(sleigh_control(Colon)).ignore_then(pcode_number_literal());
    sleigh_operation(And)
        .ignore_then(size.or_not())
        .then(whitespace_padded(expression_parser.map(Box::new)))
        .map(|(size, expression)| SASTPCodeAddressOf { expression, size })
        .map_with(spanned)
}

pub fn pcode_invocation<'tokens, P>(
    expression_parser: P
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTPCodeInvocation>, SleighParserExtra<'tokens>> + Clone
where
    P: Parser<'tokens, ParserInput<'tokens>, Spanned<SASTPCodeExpression>, SleighParserExtra<'tokens>>
        + Clone
        + 'tokens,
{
    // Simple recovery. Skips everything until a comma or closing parenthesis is discovered.
    // This will fail to recover from errors in nested expressions with parentheses.
    let arg_recovery = none_of([SleighToken::Control(CloseParen), SleighToken::Control(Comma)])
        .repeated()
        .to_span()
        .map(SASTPCodeExpression::Error)
        .map_with(spanned);

    let argument_list = expression_parser
        .clone()
        .recover_with(via_parser(arg_recovery))
        .separated_by(sleigh_control(Comma))
        .collect::<Vec<_>>();

    let no_arguments = sleigh_control(OpenParen)
        .ignore_then(sleigh_control(CloseParen))
        .map(|_| Vec::new());

    whitespace_padded(sast_ident_spanned())
        .then(no_arguments.or(parenthesized(argument_list)))
        .map(|(name, arguments)| SASTPCodeInvocation { name, arguments })
        .map_with(spanned)
        .boxed()
}
