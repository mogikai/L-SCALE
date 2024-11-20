use chumsky::{
    IterParser,
    Parser,
};

use crate::compiler::ast::Spanned;
use crate::compiler::ast::attach::SASTValue;
use crate::compiler::lexer::ParserInput;
use crate::compiler::parser::helpers::spanned;
use crate::compiler::parser::{
    SleighParserExtra,
    sast_ident_spanned,
    sleigh_control,
    sleigh_keyword,
    sleigh_number,
    sleigh_whitespace,
};
use crate::compiler::tokens::{
    SleighControlCharacter,
    SleighKeyword,
};

pub(super) fn value<'tokens>()
-> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTValue>, SleighParserExtra<'tokens>> {
    let field_list = sast_ident_spanned()
        .map(|ident| vec![ident])
        .or(sast_ident_spanned()
            .padded_by(sleigh_whitespace())
            .repeated()
            .collect::<Vec<_>>()
            .delimited_by(
                sleigh_control(SleighControlCharacter::OpenBracket),
                sleigh_control(SleighControlCharacter::CloseBracket),
            ))
        .boxed();

    let value_list = sleigh_number()
        .map_with(spanned)
        .map(|num| vec![num])
        .or(sleigh_number()
            .map_with(spanned)
            .padded_by(sleigh_whitespace())
            .repeated()
            .collect::<Vec<_>>()
            .delimited_by(
                sleigh_control(SleighControlCharacter::OpenBracket),
                sleigh_control(SleighControlCharacter::CloseBracket),
            ))
        .boxed();

    sleigh_keyword(SleighKeyword::Values)
        .padded_by(sleigh_whitespace())
        .ignore_then(field_list)
        .padded_by(sleigh_whitespace())
        .then(value_list)
        .map(|(fields, values)| SASTValue { fields, values })
        .map_with(spanned)
        .boxed()
}
