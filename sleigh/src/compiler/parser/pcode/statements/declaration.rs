use chumsky::Parser;

use crate::compiler::ast::pcode::statement::SASTPCodeStatement;
use crate::compiler::ast::Spanned;
use crate::compiler::lexer::ParserInput;
use crate::compiler::parser::helpers::spanned;
use crate::compiler::parser::pcode::atomic_expressions::{
    pcode_ident,
    pcode_number_literal,
};
use crate::compiler::parser::{
    sleigh_control,
    sleigh_keyword,
    whitespace_padded,
    SleighParserExtra,
};
use crate::compiler::tokens::{
    SleighControlCharacter,
    SleighKeyword,
};

pub fn pcode_declaration_statement<'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTPCodeStatement>, SleighParserExtra<'tokens>> + Clone {
    // Parse a declaration of the form `local some_ident;` or alternatively with a size specifier:
    // `local some_ident_with_size:4;`
    whitespace_padded(sleigh_keyword(SleighKeyword::Local))
        .ignore_then(pcode_ident())
        .then(
            sleigh_control(SleighControlCharacter::Colon)
                .ignore_then(pcode_number_literal())
                .or_not(),
        )
        .then_ignore(sleigh_control(SleighControlCharacter::SemiColon))
        .map(|(ident, size)| SASTPCodeStatement::Declaration(ident, size))
        .map_with(spanned)
        .boxed()
}
