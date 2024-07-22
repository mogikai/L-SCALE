use crate::compiler::ast::item::SASTItem;
use crate::compiler::lexer::ParserInput;
use crate::compiler::parser::{
    whitespace_padded,
    SleighParserExtra,
};
use crate::compiler::tokens::{
    IncludedTokens,
    SleighToken,
};
use chumsky::prelude::via_parser;
use chumsky::{
    select,
    select_ref,
    Parser,
};

pub fn included_tokens<'tokens, P>(
    items_parser: P
) -> impl Parser<'tokens, ParserInput<'tokens>, Vec<SASTItem>, SleighParserExtra<'tokens>>
where
    P: Parser<'tokens, ParserInput<'tokens>, Vec<SASTItem>, SleighParserExtra<'tokens>> + Clone,
{
    let tokens = select_ref! { SleighToken::IncludedTokens(IncludedTokens::Loaded(tokens)) => tokens.spanned() };
    items_parser
        .nested_in(whitespace_padded(tokens))
        .recover_with(via_parser(
            select! { SleighToken::IncludedTokens(_) => () }
                .to_span()
                .map(|span| vec![SASTItem::Error(span)]),
        ))
}
