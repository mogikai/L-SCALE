use mogi_sleigh::compiler::builder::SleighCompilerBuilder;
use mogi_sleigh::preprocessor::SleighPreprocessorFactory;
use std::path::Path;
use mogi_sleigh::compiler::error::SleighCompilerError;

mod common;

#[test]
fn test_errors() {
    common::setup_test();
    let spec_path = Path::new("./tests/assets/errors.sinc");
    let spec_dir = spec_path.parent().unwrap();
    let factory = SleighPreprocessorFactory::new().working_dir(spec_dir.to_path_buf());
    let compiler = SleighCompilerBuilder::new()
        .preprocessor_factory(factory)
        .build();
    let result = compiler.compile(spec_path);
    if let Err(SleighCompilerError::SemanticError(s)) = result {
        println!("{}", s);
    }
}