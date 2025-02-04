use crate::Spanned;
use crate::ast::define::SASTUserPcodeOp;
use crate::lexer::ParserInput;
use crate::parser::helpers::spanned;
use crate::parser::{
    SleighParserExtra,
    sast_ident_spanned,
    sleigh_keyword,
    sleigh_whitespace,
};
use crate::tokens::SleighKeyword;
use chumsky::Parser;

pub fn user_pcode_op<'tokens>()
-> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTUserPcodeOp>, SleighParserExtra<'tokens>> {
    sleigh_keyword(SleighKeyword::PCodeOp)
        .padded_by(sleigh_whitespace())
        .ignore_then(sast_ident_spanned())
        .map(|name| SASTUserPcodeOp { name })
        .map_with(spanned)
}
