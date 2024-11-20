use chumsky::Parser;

use crate::compiler::ast::Spanned;
use crate::compiler::ast::constructor::semantic_section::SASTSemanticSection;
use crate::compiler::lexer::ParserInput;
use crate::compiler::parser::helpers::spanned;
use crate::compiler::parser::pcode::pcode;
use crate::compiler::parser::{
    SleighParserExtra,
    sleigh_control,
};
use crate::compiler::tokens::SleighControlCharacter;

pub fn semantic_section<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTSemanticSection>, SleighParserExtra<'tokens>> {
    pcode()
        .delimited_by(
            sleigh_control(SleighControlCharacter::OpenBrace),
            sleigh_control(SleighControlCharacter::CloseBrace),
        )
        .map(SASTSemanticSection)
        .map_with(spanned)
        .boxed()
}
