use chumsky::{
    select_ref,
    Parser,
};

use crate::compiler::ast::item::SASTItem;
use crate::compiler::lexer::ParserInput;
use crate::compiler::parser::{
    items_parser,
    whitespace_padded,
    SleighParserExtra,
};
use crate::compiler::tokens::{
    IncludedTokens,
    SleighToken,
};

pub fn included_tokens<'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Vec<SASTItem>, SleighParserExtra<'tokens>> {
    let tokens = select_ref! { SleighToken::IncludedTokens(IncludedTokens::Loaded(tokens)) => tokens.spanned() };
    items_parser().nested_in(whitespace_padded(tokens))
}
