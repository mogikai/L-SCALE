use chumsky::error::Rich;
use chumsky::{
    extra,
    Parser,
};

use crate::compiler::ast::common::SASTIdent;
use crate::compiler::ast::pcode::subpiece::SASTPCodeSubpiece;
use crate::compiler::ast::Spanned;
use crate::compiler::lexer::{
    InputSpan,
    ParserInput,
};
use crate::compiler::parser::{
    parenthesized,
    sast_ident,
    sast_ident_spanned,
    sleigh_control,
    sleigh_number,
    sleigh_operation,
    whitespace_padded,
};
use crate::compiler::tokens::{
    SleighControlCharacter,
    SleighOperation,
    SleighToken,
};

pub(super) fn pcode_ident<'tokens, 'src: 'tokens>() -> impl Parser<
    'tokens,
    ParserInput<'tokens, 'src>,
    Spanned<'src, SASTIdent<'src>>,
    extra::Err<Rich<'tokens, SleighToken<'src>, InputSpan<'src>>>,
> + Copy {
    whitespace_padded(sast_ident_spanned())
}

pub(super) fn pcode_u16<'tokens, 'src: 'tokens>() -> impl Parser<
    'tokens,
    ParserInput<'tokens, 'src>,
    Spanned<'src, u16>,
    extra::Err<Rich<'tokens, SleighToken<'src>, InputSpan<'src>>>,
> + Copy {
    whitespace_padded(sleigh_number().try_map_with(|num, extra| {
        u16::try_from(num)
            .map(|num| (num, extra.span()))
            .map_err(|e| Rich::custom(extra.span(), e))
    }))
}

pub(super) fn pcode_subpiece<'tokens, 'src: 'tokens>() -> impl Parser<
    'tokens,
    ParserInput<'tokens, 'src>,
    Spanned<'src, SASTPCodeSubpiece<'src>>,
    extra::Err<Rich<'tokens, SleighToken<'src>, InputSpan<'src>>>,
> + Copy {
    let ident = pcode_ident();

    let size = pcode_u16();

    ident
        .then_ignore(sleigh_control(SleighControlCharacter::Colon))
        .then(size)
        .or(ident.then(parenthesized(size)))
        .map_with(|(ident, size), extra| (SASTPCodeSubpiece { ident, size }, extra.span()))
}

/// Parse a pcode label identified by `<some_label>`.
pub(super) fn pcode_label<'tokens, 'src: 'tokens>() -> impl Parser<
    'tokens,
    ParserInput<'tokens, 'src>,
    Spanned<'src, SASTIdent<'src>>,
    extra::Err<Rich<'tokens, SleighToken<'src>, InputSpan<'src>>>,
> + Copy {
    sleigh_operation(SleighOperation::LessThan)
        .ignore_then(sast_ident())
        .then_ignore(sleigh_operation(SleighOperation::GreaterThan))
        .map_with(|ident, extra| (ident, extra.span()))
}
