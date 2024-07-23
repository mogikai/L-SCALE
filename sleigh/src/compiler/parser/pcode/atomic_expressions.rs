use chumsky::error::Rich;
use chumsky::Parser;
use std::fmt::{
    Debug,
    Display,
};

use crate::compiler::ast::common::SASTIdent;
use crate::compiler::ast::pcode::bitrange::SASTPCodeBitRange;
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
    Comma,
    OpenBracket,
};
use crate::compiler::tokens::{
    SleighControlCharacter,
    SleighOperation,
};

pub(super) fn pcode_ident<'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTIdent>, SleighParserExtra<'tokens>> + Clone {
    whitespace_padded(sast_ident_spanned())
}

/// Parse a number literal and convert it into T. Surrounding whitespace will be consumed.
pub(super) fn pcode_number_literal<'tokens, T>(
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

pub(super) fn pcode_subpiece<'tokens>(
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

pub(super) fn pcode_bitrange<'tokens>(
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
pub(super) fn pcode_label<'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTIdent>, SleighParserExtra<'tokens>> + Clone {
    sleigh_operation(SleighOperation::LessThan)
        .ignore_then(sast_ident())
        .then_ignore(sleigh_operation(SleighOperation::GreaterThan))
        .map_with(spanned)
}
