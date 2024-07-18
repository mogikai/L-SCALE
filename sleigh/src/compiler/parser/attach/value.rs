use chumsky::{
    IterParser,
    Parser,
};

use crate::compiler::ast::attach::{
    SASTValue,
    SASTVariable,
};
use crate::compiler::ast::Spanned;
use crate::compiler::lexer::ParserInput;
use crate::compiler::parser::{
    sast_ident_spanned,
    sleigh_control,
    sleigh_keyword,
    sleigh_number,
    sleigh_whitespace,
    SleighParserExtra,
};
use crate::compiler::tokens::{
    SleighControlCharacter,
    SleighKeyword,
};

pub(super) fn value<'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTValue>, SleighParserExtra<'tokens>> {
    let open_bracket = sleigh_control(SleighControlCharacter::OpenBracket);
    let close_bracket = sleigh_control(SleighControlCharacter::CloseBracket);

    let fieldlist = sast_ident_spanned()
        .map(|(ident, span)| vec![(ident, span)])
        .or(sast_ident_spanned()
            .padded_by(sleigh_whitespace())
            .repeated()
            .collect::<Vec<_>>()
            .delimited_by(open_bracket, close_bracket));

    let valuelist = sleigh_number()
        .map_with(|num, extra| vec![(num, extra.span())])
        .or(sleigh_number()
            .map_with(|num, extra| (num, extra.span()))
            .padded_by(sleigh_whitespace())
            .repeated()
            .collect::<Vec<_>>()
            .delimited_by(open_bracket, close_bracket));

    sleigh_keyword(SleighKeyword::Values)
        .padded_by(sleigh_whitespace())
        .ignore_then(fieldlist)
        .padded_by(sleigh_whitespace())
        .then(valuelist)
        .map_with(|(fields, values), extra| (SASTValue { fields, values }, extra.span()))
}
