use chumsky::Parser;

use crate::compiler::ast::Spanned;
use crate::compiler::ast::define::SASTUserPcodeOp;
use crate::compiler::lexer::ParserInput;
use crate::compiler::parser::helpers::spanned;
use crate::compiler::parser::{
    SleighParserExtra,
    sast_ident_spanned,
    sleigh_keyword,
    sleigh_whitespace,
};
use crate::compiler::tokens::SleighKeyword;

pub(super) fn user_pcode_op<'tokens>()
-> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTUserPcodeOp>, SleighParserExtra<'tokens>> {
    sleigh_keyword(SleighKeyword::PCodeOp)
        .padded_by(sleigh_whitespace())
        .ignore_then(sast_ident_spanned())
        .map(|name| SASTUserPcodeOp { name })
        .map_with(spanned)
}
