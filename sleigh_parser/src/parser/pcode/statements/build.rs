use crate::Spanned;
use crate::ast::pcode::statement::SASTPCodeStatement;
use crate::lexer::ParserInput;
use crate::parser::helpers::spanned;
use crate::parser::{
    SleighParserExtra,
    sast_ident_spanned,
    sleigh_control,
    sleigh_keyword,
    whitespace_padded,
};
use crate::tokens::SleighControlCharacter;
use crate::tokens::SleighKeyword::Build;
use chumsky::Parser;

/// A build statement
/// `build ident;`: Execute the p-code to build operand ident.
pub fn pcode_build_statement<'tokens>()
-> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTPCodeStatement>, SleighParserExtra<'tokens>> + Clone {
    whitespace_padded(sleigh_keyword(Build))
        .ignore_then(whitespace_padded(sast_ident_spanned()))
        .then_ignore(sleigh_control(SleighControlCharacter::SemiColon))
        .map(SASTPCodeStatement::Build)
        .map_with(spanned)
}
