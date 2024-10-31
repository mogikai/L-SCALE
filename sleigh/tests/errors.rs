use mogi_sleigh::compiler::error::SleighCompilerError;
use crate::common::build;

mod common;

const ASSETS_PATH: &str = "./tests/assets/";
const SPACE_ATTRS_PATH: &str = "./tests/assets/duplicate_space_attrs/";

#[test]
fn duplicate_space_type() {
    common::setup_test();
    let path = SPACE_ATTRS_PATH.to_string() + "type.sinc";
    let err = build(path).expect_err("expected error");
    assert!(matches!(err, SleighCompilerError::SemanticError(_)));
    if let SleighCompilerError::SemanticError(string) = err {
        assert!(string.contains("Space attribute 'Type(RamSpace)' is duplicated"))
    }
}

#[test]
fn duplicate_space_size() {
    common::setup_test();
    let path = SPACE_ATTRS_PATH.to_string() + "size.sinc";
    let err = build(path).expect_err("expected error");
    assert!(matches!(err, SleighCompilerError::SemanticError(_)));
    if let SleighCompilerError::SemanticError(string) = err {
        assert!(string.contains("Space attribute 'Size(20)' is duplicated"))
    }
}

#[test]
fn duplicate_space_wordsize() {
    common::setup_test();
    let path = SPACE_ATTRS_PATH.to_string() + "wordsize.sinc";
    let err = build(path).expect_err("expected error");
    assert!(matches!(err, SleighCompilerError::SemanticError(_)));
    if let SleighCompilerError::SemanticError(string) = err {
        assert!(string.contains("Space attribute 'Wordsize(10)' is duplicated"))
    }
}

#[test]
fn duplicate_space_default() {
    common::setup_test();
    let path = SPACE_ATTRS_PATH.to_string() + "default.sinc";
    let err = build(path).expect_err("expected error");
    assert!(matches!(err, SleighCompilerError::SemanticError(_)));
    if let SleighCompilerError::SemanticError(string) = err {
        assert!(string.contains("Space attribute 'IsDefault' is duplicated"))
    }
}

#[test]
fn duplicate_define() {
    common::setup_test();
    let path = ASSETS_PATH.to_string() + "duplicate_define.sinc";
    let err = build(path).expect_err("expected error");
    assert!(matches!(err, SleighCompilerError::SemanticError(_)));
    if let SleighCompilerError::SemanticError(string) = err {
        assert!(string.contains("Definition of Endianness already exists"))
    }
}

#[test]
fn symbol_undeclared() {
    common::setup_test();
    let path = ASSETS_PATH.to_string() + "symbol_undeclared.sinc";
    let err = build(path).expect_err("expected error");
    assert!(matches!(err, SleighCompilerError::SemanticError(_)));
    if let SleighCompilerError::SemanticError(string) = err {
        assert!(string.contains("The symbol was not declared before being defined"))
    }
}

#[test]
fn invalid_field_range() {
    common::setup_test();
    let path = ASSETS_PATH.to_string() + "invalid_field_range.sinc";
    let err = build(path).expect_err("expected error");
    assert!(matches!(err, SleighCompilerError::SemanticError(_)));
    if let SleighCompilerError::SemanticError(string) = err {
        assert!(string.contains("field range is too large"));
        assert!(string.contains("field range ends before its starting"));
    }
}

#[test]
fn slice_error() {
    common::setup_test();
    let path = ASSETS_PATH.to_string() + "slice_error.sinc";
    let err = build(path).expect_err("expected error");
    assert!(matches!(err, SleighCompilerError::SemanticError(_)));
    if let SleighCompilerError::SemanticError(string) = err {
        assert!(string.contains("value cannot be sliced as the size is undetermined"));
        // TODO implement and test other slice errors
    }
}

#[test]
fn instruction_after_export() {
    common::setup_test();
    let path = ASSETS_PATH.to_string() + "instruction_after_export.sinc";
    let err = build(path).expect_err("expected error");
    assert!(matches!(err, SleighCompilerError::SemanticError(_)));
    if let SleighCompilerError::SemanticError(string) = err {
        assert!(string.contains("Instruction after export"));
        // TODO implement and test other slice errorsy
    }
}
