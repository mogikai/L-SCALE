use chumsky::{
    IterParser,
    Parser,
};

use crate::compiler::ast::attach::SASTName;
use crate::compiler::ast::Spanned;
use crate::compiler::lexer::ParserInput;
use crate::compiler::parser::{
    sast_ident_spanned,
    sleigh_control,
    sleigh_keyword,
    sleigh_whitespace,
    SleighParserExtra,
};
use crate::compiler::tokens::{
    SleighControlCharacter,
    SleighKeyword,
};

pub(super) fn name<'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTName>, SleighParserExtra<'tokens>> {
    let stringlist = sast_ident_spanned()
        .map(|(ident, span)| vec![(ident, span)])
        .or(sast_ident_spanned()
            .padded_by(sleigh_whitespace())
            .repeated()
            .collect::<Vec<_>>()
            .delimited_by(
                sleigh_control(SleighControlCharacter::OpenBracket),
                sleigh_control(SleighControlCharacter::CloseBracket),
            ));

    sleigh_keyword(SleighKeyword::Names)
        .padded_by(sleigh_whitespace())
        .ignore_then(stringlist.clone()) // registers
        .padded_by(sleigh_whitespace())
        .then(stringlist) // alternative strings
        .map_with(|(fields, strings), extra| (SASTName { fields, strings }, extra.span()))
}
