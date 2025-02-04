use crate::Spanned;
use crate::ast::constructor::semantic_section::SASTSemanticSection;
use crate::lexer::ParserInput;
use crate::parser::helpers::spanned;
use crate::parser::pcode::pcode;
use crate::parser::{
    SleighParserExtra,
    sleigh_control,
};
use crate::tokens::SleighControlCharacter;
use chumsky::Parser;

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
