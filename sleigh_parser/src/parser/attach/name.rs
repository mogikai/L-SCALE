use crate::Spanned;
use crate::ast::attach::SASTName;
use crate::lexer::ParserInput;
use crate::parser::helpers::spanned;
use crate::parser::{
    SleighParserExtra,
    sast_ident_spanned,
    sleigh_control,
    sleigh_keyword,
    sleigh_whitespace,
};
use crate::tokens::{
    SleighControlCharacter,
    SleighKeyword,
};
use chumsky::{
    IterParser,
    Parser,
};

pub fn name<'tokens>() -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTName>, SleighParserExtra<'tokens>> {
    let stringlist = sast_ident_spanned()
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

    sleigh_keyword(SleighKeyword::Names)
        .padded_by(sleigh_whitespace())
        .ignore_then(stringlist.clone()) // registers
        .padded_by(sleigh_whitespace())
        .then(stringlist) // alternative strings
        .map(|(fields, strings)| SASTName { fields, strings })
        .map_with(spanned)
        .boxed()
}
