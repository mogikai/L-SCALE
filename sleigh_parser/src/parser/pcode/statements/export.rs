use crate::Spanned;
use crate::ast::pcode::statement::SASTPCodeStatement;
use crate::lexer::ParserInput;
use crate::parser::helpers::spanned;
use crate::parser::pcode::expression::pcode_expression;
use crate::parser::{
    SleighParserExtra,
    sleigh_control,
    sleigh_keyword,
    whitespace_padded,
};
use crate::tokens::SleighControlCharacter;
use crate::tokens::SleighKeyword::Export;
use chumsky::Parser;

/// An export statement
/// `export IDENT;`: Export the identifier. Can also be prefixed by a `&` to export the address.
pub fn pcode_export_statement<'tokens>()
-> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTPCodeStatement>, SleighParserExtra<'tokens>> + Clone {
    whitespace_padded(sleigh_keyword(Export))
        .ignore_then(pcode_expression())
        .then_ignore(sleigh_control(SleighControlCharacter::SemiColon))
        .map(SASTPCodeStatement::Export)
        .map_with(spanned)
}
