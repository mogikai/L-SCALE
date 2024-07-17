use chumsky::error::Rich;
use chumsky::Parser;

use crate::compiler::ast::common::SASTIdent;
use crate::compiler::ast::pcode::bitrange::SASTPCodeBitRange;
use crate::compiler::ast::pcode::subpiece::SASTPCodeSubpiece;
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

pub(super) fn pcode_ident<'tokens, 'src: 'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTIdent>, SleighParserExtra<'tokens>> + Copy {
    whitespace_padded(sast_ident_spanned())
}

pub(super) fn pcode_u16<'tokens, 'src: 'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<u16>, SleighParserExtra<'tokens>> + Copy {
    whitespace_padded(
        sleigh_number()
            .try_map_with(|num, extra| u16::try_from(num).map_err(|e| Rich::custom(extra.span(), e)))
            .map_with(spanned),
    )
}

pub(super) fn pcode_subpiece<'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTPCodeSubpiece>, SleighParserExtra<'tokens>> + Copy {
    let ident = pcode_ident();

    let size = pcode_u16();

    ident
        .then_ignore(sleigh_control(SleighControlCharacter::Colon))
        .then(size)
        .or(ident.then(parenthesized(size)))
        .map(|(ident, size)| SASTPCodeSubpiece { ident, size })
        .map_with(spanned)
}

pub(super) fn pcode_bitrange<'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTPCodeBitRange>, SleighParserExtra<'tokens>> + Copy {
    let lsb = pcode_u16();
    let bits = pcode_u16();
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
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTIdent>, SleighParserExtra<'tokens>> + Copy {
    sleigh_operation(SleighOperation::LessThan)
        .ignore_then(sast_ident())
        .then_ignore(sleigh_operation(SleighOperation::GreaterThan))
        .map_with(spanned)
}
