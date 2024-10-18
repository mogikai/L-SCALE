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

const SPACE_ATTRS_PATH: &str = "./tests/assets/duplicate_space_attrs/";

// TODO fix tests

#[test]
#[ignore]
fn duplicate_space_type() {
    let path = SPACE_ATTRS_PATH.to_string() + "type.sinc";
    let spec_path = Path::new(&path);
    let spec_dir = spec_path.parent().unwrap();
    let factory = SleighPreprocessorFactory::new().working_dir(spec_dir.to_path_buf());
    let compiler = SleighCompilerBuilder::new()
        .preprocessor_factory(factory)
        .build();
    let err = compiler.compile(spec_path).expect_err("expected error");
    assert!(matches!(err, SleighCompilerError::SemanticError(_)));
    if let SleighCompilerError::SemanticError(string) = err {
        assert!(string.contains("Space attribute 'Type(RamSpace)' is already defined."))
    }
}

#[test]
#[ignore]
fn duplicate_space_size() {
    let path = SPACE_ATTRS_PATH.to_string() + "size.sinc";
    let spec_path = Path::new(&path);
    let spec_dir = spec_path.parent().unwrap();
    let factory = SleighPreprocessorFactory::new().working_dir(spec_dir.to_path_buf());
    let compiler = SleighCompilerBuilder::new()
        .preprocessor_factory(factory)
        .build();
    let err = compiler.compile(spec_path).expect_err("expected error");
    assert!(matches!(err, SleighCompilerError::AtomizeStatementFailed(_)));
    if let SleighCompilerError::SemanticError(string) = err {
        assert!(string.contains("Space attribute 'Size(20)' is already defined."))
    }
}

#[test]
#[ignore]
fn duplicate_space_wordsize() {
    let path = SPACE_ATTRS_PATH.to_string() + "wordsize.sinc";
    let spec_path = Path::new(&path);
    let spec_dir = spec_path.parent().unwrap();
    let factory = SleighPreprocessorFactory::new().working_dir(spec_dir.to_path_buf());
    let compiler = SleighCompilerBuilder::new()
        .preprocessor_factory(factory)
        .build();
    let err = compiler.compile(spec_path).expect_err("expected error");
    assert!(matches!(err, SleighCompilerError::SemanticError(_)));
    if let SleighCompilerError::SemanticError(string) = err {
        assert!(string.contains("Space attribute 'Wordsize(10)' is already defined."))
    }
}

#[test]
#[ignore]
fn duplicate_space_default() {
    let path = SPACE_ATTRS_PATH.to_string() + "default.sinc";
    let spec_path = Path::new(&path);
    let spec_dir = spec_path.parent().unwrap();
    let factory = SleighPreprocessorFactory::new().working_dir(spec_dir.to_path_buf());
    let compiler = SleighCompilerBuilder::new()
        .preprocessor_factory(factory)
        .build();
    let err = compiler.compile(spec_path).expect_err("expected error");
    assert!(matches!(err, SleighCompilerError::SemanticError(_)));
    if let SleighCompilerError::SemanticError(string) = err {
        assert!(string.contains("Space attribute 'IsDefault' is already defined."))
    }
}