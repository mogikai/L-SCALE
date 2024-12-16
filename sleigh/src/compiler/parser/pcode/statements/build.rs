use chumsky::Parser;

use crate::compiler::ast::pcode::statement::SASTPCodeStatement;
use crate::compiler::ast::Spanned;
use crate::compiler::lexer::ParserInput;
use crate::compiler::parser::helpers::spanned;
use crate::compiler::parser::{
    sast_ident_spanned,
    sleigh_control,
    sleigh_keyword,
    whitespace_padded,
    SleighParserExtra,
};
use crate::compiler::tokens::SleighControlCharacter;
use crate::compiler::tokens::SleighKeyword::Build;

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
