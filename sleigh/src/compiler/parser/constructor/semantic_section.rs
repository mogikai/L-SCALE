use crate::compiler::ast::constructor::semantic_section::SASTSemanticSection;
use crate::compiler::ast::pcode::SASTPCode;
use crate::compiler::ast::Spanned;
use crate::compiler::lexer::{
    InputSpan,
    ParserInput,
};
use crate::compiler::parser::pcode::pcode;
use crate::compiler::parser::{
    sleigh_control,
    SleighParserExtra,
};
use crate::compiler::tokens::{
    SleighControlCharacter,
    SleighToken,
};
use chumsky::error::Rich;
use chumsky::prelude::{
    any,
    just,
};
use chumsky::{
    extra,
    Parser,
};

pub(super) fn semantic_section<'tokens, 'src: 'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTSemanticSection>, SleighParserExtra<'tokens>> {
    pcode()
        .delimited_by(
            sleigh_control(SleighControlCharacter::OpenBrace),
            sleigh_control(SleighControlCharacter::CloseBrace),
        )
        .map_with(|pcode, extra| (SASTSemanticSection(pcode), extra.span()))
}
