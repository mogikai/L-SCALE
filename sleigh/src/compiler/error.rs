use std::collections::HashMap;
use crate::compiler::inputcache::InputCacheError;
use crate::compiler::lexer::InputSpan;
use crate::preprocessor::error::PreprocessorError;
use crate::spec::builder::{MOIRSymbolId, StringId};
use crate::spec::moir::common::MOIRSize;
use crate::spec::ProcessorSpecificationError;
use std::fmt::{
    Debug,
    Display,
};
use std::ops::Range;
use ariadne::{Label, Report, ReportKind};
use chumsky::span::Span;
use thiserror::Error;
use crate::compiler::ast::define::SASTAddressSpaceAttribute;

#[derive(Debug, Error)]
pub enum SliceError {
    #[error("value cannot be sliced, since the requested slice is either too large or too small. New size: {new:?}, current size: {current:?}"
    )]
    InvalidSliceSize { new: MOIRSize, current: MOIRSize },
    #[error("value cannot be sliced, since the slice range is invalid")]
    InvalidSliceRange {
        current_start: MOIRSize,
        current_end: MOIRSize,
        new_start: MOIRSize,
        new_end: MOIRSize,
    },
    #[error("value cannot be sliced as the size is undetermined")]
    UndeterminedSize,
}

#[derive(Debug, Error)]
pub enum ConstEvalError {
    #[error("operation encountered an overflow during constant folding")]
    Overflow,
}

#[derive(Error, Debug)]
pub enum InvalidFieldRange {
    #[error("field range ends before its starting")]
    EndBeforeStart(Range<u64>),
    #[error("field range is too large")]
    TooLargeRange(u64, Range<u64>),
}

#[derive(Debug, Error)]
#[error("The constant value being constructed does not fit into the requested size. Value: {val:?}, Requested size: {size:?}"
)]
pub struct WrongConstSize {
    pub val: u128,
    pub size: MOIRSize,
}

#[derive(Debug, Error)]
#[error("The identifier is not mapped")]
pub struct UnmappedIdentifier {
    pub field: StringId
}

#[derive(Error, Debug)]
#[error("Cannot slice left hand side")]
pub struct CannotTruncateLhs {
    pub ident_str: String,
    pub expected_size: MOIRSize,
}

#[derive(Error, Debug)]
#[error("Identifier already exists")]
pub struct DuplicatedIdentifier {
    pub name: String,
}

#[derive(Error, Debug)]
#[error("Definition of {dupe_type:?} already exists")]
pub struct DuplicatedDefine {
    pub dupe_type: DuplicateDefineType,
}

#[derive(Debug)]
pub enum DuplicateDefineType {
    Endianness,
    Alignment,
}

#[derive(Error, Debug)]
#[error("Undefined symbol")]
pub struct UndefinedSymbol {
    pub symbol: String,
}

#[derive(Error, Debug)]
#[error("Invalid symbol type")]
pub struct InvalidSymbolType {
    pub symbol_reference: String,
    pub expected: String,
    pub found: MOIRSymbolId,
}

#[derive(Error, Debug)]
#[error("Undefined identifier: ")]
pub struct UndefinedIdentifier {
    pub identifier: String,
}

#[derive(Error, Debug)]
#[error("Space attribute '{attribute:?}' is duplicated")]
pub struct DuplicatedSpaceAttribute {
    pub attribute: SASTAddressSpaceAttribute,
}

#[derive(Debug, Error)]
pub enum SleighCompilerErrorType {
    #[error("The preprocessor encountered an error: {0}")]
    PreprocessorError(#[from] PreprocessorError),
    #[error("The input cache encountered an error: {0}")]
    InputCacheError(#[from] InputCacheError),
    #[error("The processor specification encountered an error: {0}")]
    ProcessorSpecificationError(#[from] ProcessorSpecificationError),
    #[error("An error occurred during lexical analysis: {0}")]
    LexerError(String),
    #[error("An error  occurred during parsing: {0}")]
    ParserError(String),
    #[error("The requested source was not found.")]
    MissingSource,
    #[error("The input is not yet tokenized.")]
    NotTokenized,
    #[error("The input is not yet preprocessed. Parsing requires a preprocessed token stream.")]
    NotPreprocessed,
    #[error("The compiler was configured to stop on the first error encountered.")]
    StopOnFirstError,
    #[error("The compiler encountered an error during the semantic analysis phase.")]
    SemanticError(String),
    #[error("Cannot slice: {0}")]
    SliceError(#[from] SliceError),
    #[error("Cannot evaluate constants: {0}")]
    ConstEvalError(#[from] ConstEvalError),
    #[error("The field range is not valid: {0}")]
    InvalidFieldRange(#[from] InvalidFieldRange),
    #[error("Const does not fit: {0}")]
    WrongConstSize(#[from] WrongConstSize),
    #[error("The symbol is already defined")]
    RedefinedSymbol,
    #[error("The builtin operation was called with an incorrect number of arguments")]
    InvalidArgumentCount,
    #[error("Identifier not mapped: {0}")]
    UnmappedIdentifier(#[from] UnmappedIdentifier),
    #[error("Cannot truncate LHS: {0}")]
    CannotTruncateLhs(#[from] CannotTruncateLhs),
    #[error("Instruction after export")]
    InstructionAfterExport,
    #[error("The symbol was not declared before being defined")]
    UndeclaredSymbol,
    #[error("Identifier exists: {0}")]
    DuplicatedIdentifier(#[from] DuplicatedIdentifier),
    #[error("Duplicate space attribute: {0}")]
    DuplicatedSpaceAttribute(#[from] DuplicatedSpaceAttribute),
    #[error("Duplicate define: {0}")]
    DuplicatedDefine(#[from] DuplicatedDefine),
    #[error("Undefined identifier: {0}")]
    UndefinedIdentifier(#[from] UndefinedIdentifier),
    #[error("Undefined symbol: {0}")]
    UndefinedSymbol(#[from] UndefinedSymbol),
    #[error("Invalid symbol type: {0}")]
    InvalidSymbolType(#[from] InvalidSymbolType),
}

#[derive(Debug, Error)]
#[error("{spans:?} {error:?}")]
pub struct SleighCompilerError {
    pub spans: HashMap<&'static str, InputSpan>,
    pub error: SleighCompilerErrorType,
    pub cause: Option<Box<SleighCompilerError>>,
}

impl From<SleighCompilerErrorType> for SleighCompilerError {
    fn from(value: SleighCompilerErrorType) -> Self {
        SleighCompilerError {
            spans: HashMap::new(),
            error: value,
            cause: None,
        }
    }
}

impl From<ProcessorSpecificationError> for SleighCompilerError {
    fn from(value: ProcessorSpecificationError) -> Self {
        let e: SleighCompilerErrorType = value.into();
        e.into()
    }
}

impl From<InputCacheError> for SleighCompilerError {
    fn from(value: InputCacheError) -> Self {
        SleighCompilerError::from_error(value.into())
    }
}

impl From<PreprocessorError> for SleighCompilerError {
    fn from(value: PreprocessorError) -> Self {
        SleighCompilerError::from_error(value.into())
    }
}

impl From<WrongConstSize> for SleighCompilerError {
    fn from(value: WrongConstSize) -> Self {
        SleighCompilerError::from_error(value.into())
    }
}

impl SleighCompilerError {
    pub fn from_error(error: SleighCompilerErrorType) -> Self {
        SleighCompilerError {
            spans: HashMap::new(),
            error: error.into(),
            cause: None,
        }
    }

    pub fn with_span(error: SleighCompilerErrorType, key: &'static str, span: InputSpan) -> Self {
        let mut spans = HashMap::new();
        spans.insert(key, span);
        SleighCompilerError {
            spans,
            error: error.into(),
            cause: None,
        }
    }

    pub fn with_spans(error: SleighCompilerErrorType, spans: HashMap<&'static str, InputSpan>) -> Self {
        SleighCompilerError {
            spans,
            error: error.into(),
            cause: None,
        }
    }

    pub fn span(mut self, key: &'static str, span: InputSpan) -> Self {
        self.spans.insert(key, span);
        self
    }

    pub fn cause(mut self, cause: SleighCompilerError) -> Self {
        match self.cause {
            None => self.cause = Some(Box::new(cause)),
            Some(_) => todo!(),
        }
        self
    }

    fn get_span(&self, key: &'static str) -> InputSpan {
        self.spans.get(key).expect(format!("key '{key}' should exist in error spans").as_str()).clone()
    }
}

impl SleighCompilerError {
    pub fn as_report(&self) -> Report<'static, InputSpan> {
        (match &self.error {
            SleighCompilerErrorType::DuplicatedDefine(_) | SleighCompilerErrorType::DuplicatedSpaceAttribute(_) => {
                Report::build(ReportKind::Error,
                              self.get_span("first").context(),
                              self.get_span("first").start())
                    .with_label(Label::new(self.get_span("first"))
                        .with_message("first definition"))
                    .with_label(Label::new(self.get_span("second"))
                        .with_message("second definition"))
            }
            SleighCompilerErrorType::InstructionAfterExport => {
                Report::build(ReportKind::Error,
                              self.get_span("instr").context(),
                              self.get_span("instr").start())
                    .with_label(Label::new(self.get_span("export"))
                        .with_message("export is here"))
                    .with_label(Label::new(self.get_span("instr"))
                        .with_message("instruction should be before export"))
            }
            SleighCompilerErrorType::InvalidFieldRange(invalid) => {
                match invalid {
                    InvalidFieldRange::EndBeforeStart(range) => {
                        Report::build(ReportKind::Error,
                                      self.get_span("field").context(),
                                      self.get_span("field").start())
                            .with_label(Label::new(self.get_span("field").clone())
                            .with_message(format!("range ends at {} but starts at {}", range.end, range.start)))
                    }
                    InvalidFieldRange::TooLargeRange(bits, range) => {
                        Report::build(ReportKind::Error,
                                      self.get_span("field").context(),
                                      self.get_span("field").start())
                            .with_label(Label::new(self.get_span("field").clone()).
                            with_message(format!("range fits {} bits, but there are only {} bits",
                                                 range.end - range.start, bits)))
                    }
                }
            }
            SleighCompilerErrorType::SliceError(slice) => {
                match slice {
                    SliceError::UndeterminedSize => {
                        Report::build(ReportKind::Error,
                                      self.get_span("expr").context(),
                                      self.get_span("expr").start())
                            .with_label(Label::new(self.get_span("expr").clone())
                                .with_message("size is undetermined"))
                    }
                    _ => todo!("add more slice error reports")
                }
            }
            SleighCompilerErrorType::UndeclaredSymbol => {
                Report::build(ReportKind::Error,
                              self.get_span("ident").context(),
                              self.get_span("ident").start())
                    .with_label(Label::new(self.get_span("ident").clone())
                    .with_message("symbol is undeclared"))
            }
            _ => todo!("unhandled error in spanned_report")
        })
            .with_message(self.error.to_string())
            // TODO include cause
            .finish()
    }
}
