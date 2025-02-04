use crate::inputcache::CompilerInputCache;
use crate::lexer::{
    SleighLexer,
    SleighTokens,
};
pub use crate::parser::report_errors;
use crate::{
    ParseError,
    SleighInterner,
};

pub type TestResult<T = ()> = Result<T, ParseError>;

#[cfg(test)]
pub fn setup_parser_test(test_string: &str) -> TestResult<(CompilerInputCache, SleighInterner, SleighTokens)> {
    let lexer = SleighLexer::default();
    let mut input_cache = CompilerInputCache::default();
    let id = input_cache
        .load_input_from_str("test", test_string)
        // We just want to fix the type, not generate a proper error report
        .map_err(|e| ParseError::LexerError(e.to_string()))?;
    let mut context = Default::default();
    let tokens = lexer.tokenize(&mut input_cache, &mut context, id)?;
    Ok((input_cache, context, tokens))
}

#[cfg(test)]
pub fn test_parser<O, P>(
    test_string: &str,
    parse_func: P,
) -> TestResult<O>
where
    P: Fn(&mut CompilerInputCache, &mut SleighInterner, SleighTokens) -> TestResult<O>,
{
    let (mut cache, mut context, tokens) = setup_parser_test(test_string)?;
    parse_func(&mut cache, &mut context, tokens)
}
