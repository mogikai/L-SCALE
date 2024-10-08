use mogi_sleigh::compiler::builder::SleighCompilerBuilder;
use mogi_sleigh::preprocessor::SleighPreprocessorFactory;
use std::path::Path;
use mogi_sleigh::compiler::error::SleighCompilerError;

#[test]
fn compiler_test() {
    let spec_path = Path::new("./tests/assets/error.sinc");
    let spec_dir = spec_path.parent().unwrap();
    let factory = SleighPreprocessorFactory::new().working_dir(spec_dir.to_path_buf());
    let compiler = SleighCompilerBuilder::new()
        .preprocessor_factory(factory)
        .build();
    compiler.compile(spec_path).unwrap();
}

fn duplicate_space_attr(attr: &str) {
    let path = format!("./tests/assets/duplicate_space_attrs/{attr}.sinc");
    let spec_path = Path::new(&path);
    let spec_dir = spec_path.parent().unwrap();
    let factory = SleighPreprocessorFactory::new().working_dir(spec_dir.to_path_buf());
    let compiler = SleighCompilerBuilder::new()
        .preprocessor_factory(factory)
        .build();
    let err = compiler.compile(spec_path).expect_err("expected error");
    assert!(matches!(err, SleighCompilerError::SemanticError(_)));
    if let SleighCompilerError::SemanticError(string) = err {
        assert!(string.contains(&format!("Space attribute '{attr}' defined two times.")))
    }
}

#[test]
fn duplicate_space_type() {
    duplicate_space_attr("type");
}

#[test]
fn duplicate_space_size() {
    duplicate_space_attr("size");
}

#[test]
fn duplicate_space_wordsize() {
    duplicate_space_attr("wordsize");
}

#[test]
fn duplicate_space_default() {
    duplicate_space_attr("default");
}