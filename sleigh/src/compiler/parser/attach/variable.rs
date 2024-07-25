use chumsky::{
    IterParser,
    Parser,
};

use crate::compiler::ast::attach::SASTVariable;
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

pub(super) fn variable<'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTVariable>, SleighParserExtra<'tokens>> {
    let fieldlist = sast_ident_spanned()
        .map(|(ident, span)| vec![(ident, span)])
        .or(sast_ident_spanned()
            .padded_by(sleigh_whitespace())
            .repeated()
            .collect::<Vec<_>>()
            .delimited_by(
                sleigh_control(SleighControlCharacter::OpenBracket),
                sleigh_control(SleighControlCharacter::CloseBracket),
            ));

    sleigh_keyword(SleighKeyword::Variables)
        .padded_by(sleigh_whitespace())
        .ignore_then(fieldlist.clone())
        .padded_by(sleigh_whitespace())
        .then(fieldlist)
        .map_with(|(fields, registers), extra| (SASTVariable { fields, registers }, extra.span()))
}
