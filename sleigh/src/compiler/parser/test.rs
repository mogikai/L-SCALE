use crate::compiler::error::SleighCompilerError;
use crate::compiler::inputcache::CompilerInputCache;
use crate::compiler::lexer::{
    SleighLexer,
    SleighTokens,
};
pub(crate) use crate::compiler::parser::report_errors;
use crate::spec::builder::ProcessorSpecificationBuilder;

pub type TestResult<T = ()> = Result<T, SleighCompilerError>;

#[cfg(test)]
pub fn setup_parser_test(
    test_string: &str
) -> TestResult<(CompilerInputCache, ProcessorSpecificationBuilder, SleighTokens)> {
    let lexer = SleighLexer::default();
    let mut input_cache = CompilerInputCache::default();
    let id = input_cache.load_input_from_str("test", test_string)?;
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
    P: Fn(&mut CompilerInputCache, &mut ProcessorSpecificationBuilder, SleighTokens) -> TestResult<O>,
{
    let (mut cache, mut context, tokens) = setup_parser_test(test_string)?;
    parse_func(&mut cache, &mut context, tokens)
}
