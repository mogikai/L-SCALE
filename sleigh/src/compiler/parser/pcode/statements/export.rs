use chumsky::Parser;

use crate::compiler::ast::pcode::statement::SASTPCodeStatement;
use crate::compiler::ast::Spanned;
use crate::compiler::lexer::ParserInput;
use crate::compiler::parser::helpers::spanned;
use crate::compiler::parser::pcode::atomic_expressions::{
    pcode_deref,
    pcode_subpiece,
};
use crate::compiler::parser::pcode::expression::pcode_expression;
use crate::compiler::parser::{
    sast_ident_spanned,
    sleigh_control,
    sleigh_keyword,
    sleigh_operation,
    whitespace_padded,
    SleighParserExtra,
};
use crate::compiler::tokens::SleighControlCharacter;
use crate::compiler::tokens::SleighKeyword::Export;
use crate::compiler::tokens::SleighOperation::And;

/// An export statement
/// `export IDENT;`: Export the identifier. Can also be prefixed by a `&` to export the address.
pub fn pcode_export_statement<'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTPCodeStatement>, SleighParserExtra<'tokens>> + Clone {
    whitespace_padded(sleigh_keyword(Export))
        .ignore_then(pcode_expression())
        .then_ignore(sleigh_control(SleighControlCharacter::SemiColon))
        .map(SASTPCodeStatement::Export)
        .map_with(spanned)
}
