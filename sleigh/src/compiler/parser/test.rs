use crate::compiler::ast::Spanned;
use crate::compiler::error::SleighCompilerError;
use crate::compiler::lexer::{
    InputSpan,
    ParserInput,
    SleighLexer,
    SleighTokens,
};
use crate::compiler::parser::{
    report_errors,
    SleighParserExtra,
};
use crate::compiler::tokens::SleighToken;
use crate::compiler::{
    CompilerContext,
    CompilerInputCache,
};
use chumsky::prelude::Rich;
use chumsky::{
    ParseResult,
    Parser,
};

#[cfg(test)]
pub fn setup_parser_test(
    test_string: &str
) -> Result<(CompilerInputCache, CompilerContext, SleighTokens), SleighCompilerError> {
    let lexer = SleighLexer::default();
    let mut input_cache = CompilerInputCache::default();
    let id = input_cache.load_input_from_str("test", test_string)?;
    let mut context = CompilerContext::default();
    let tokens = lexer.tokenize(&mut input_cache, &mut context, id)?;
    Ok((input_cache, context, tokens))
}

#[cfg(test)]
pub fn test_parser<O, P>(
    test_string: &str,
    parse_func: P,
) -> Result<O, SleighCompilerError>
where
    P: Fn(&mut CompilerInputCache, &mut CompilerContext, SleighTokens) -> Result<O, SleighCompilerError>,
{
    let (mut cache, mut context, tokens) = setup_parser_test(test_string)?;
    parse_func(&mut cache, &mut context, tokens)
}
