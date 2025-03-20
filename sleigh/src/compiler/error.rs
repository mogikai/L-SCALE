use crate::spec::ProcessorSpecificationError;
use crate::spec::builder::MOIRSymbolId;
use crate::spec::moir::common::MOIRSize;
use ariadne::{
    ColorGenerator,
    Fmt,
    Label,
    Report,
    ReportKind,
};
use mogi_sleigh_parser::ast::define::SASTAddressSpaceAttribute;
use mogi_sleigh_parser::inputcache::CompilerInputCache;
use mogi_sleigh_parser::lexer::InputSpan;
use mogi_sleigh_parser::preprocessor::error::PreprocessorError;
use mogi_sleigh_parser::{
    ParseError,
    StringId,
};
use std::collections::HashMap;
use std::fmt::{
    Debug,
    Display,
    Formatter,
};
use std::ops::Range;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SliceError {
    #[error(
        "value cannot be sliced, since the requested slice is either too large or too small. New size: {new:?}, current size: {current:?}"
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
#[error(
    "The constant value being constructed does not fit into the requested size. Value: {val:?}, Requested size: {size:?}"
)]
pub struct WrongConstSize {
    pub val: u128,
    pub size: MOIRSize,
}

#[derive(Debug, Error)]
#[error("The identifier is not mapped")]
pub struct UnmappedIdentifier {
    pub field: StringId,
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
#[error("Symbol {symbol} is not defined")]
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
#[error("Undefined identifier")]
pub struct UndefinedIdentifier {
    pub identifier: String,
}

#[derive(Error, Debug)]
#[error("Space attribute '{attribute:?}' is duplicated")]
pub struct DuplicatedSpaceAttribute {
    pub attribute: SASTAddressSpaceAttribute,
}

#[derive(Debug, Error)]
pub enum InternalErrorType {
    #[error("The preprocessor encountered an error: {0}")]
    PreprocessorError(#[from] PreprocessorError),
    #[error("The processor specification encountered an error: {0}")]
    ProcessorSpecificationError(#[from] ProcessorSpecificationError),
    #[error("The compiler was configured to stop on the first error encountered.")]
    StopOnFirstError,
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
    #[error("An unknown error occurred.")]
    Unknown,
}

#[derive(Debug, Error)]
#[error("{spans:?} {error:?}")]
pub struct InternalError {
    /// A hashmap to associate spans to identifiers defined by certain error types.
    /// This makes it possible to add the span context at later stages where it might
    /// be available, where it is not available during construction of the error.
    pub spans: HashMap<&'static str, InputSpan>,
    /// The actual type of the error.
    pub error: InternalErrorType,
    /// If the error is caused by another error, this other error can be provided here.
    pub cause: Option<Box<InternalError>>,
}

impl From<InternalErrorType> for InternalError {
    fn from(error: InternalErrorType) -> Self {
        InternalError {
            spans: HashMap::new(),
            error,
            cause: None,
        }
    }
}

impl From<ProcessorSpecificationError> for InternalError {
    fn from(value: ProcessorSpecificationError) -> Self {
        InternalError::from_error(value.into())
    }
}

impl InternalError {
    pub fn from_error(error: InternalErrorType) -> Self {
        InternalError {
            spans: HashMap::new(),
            error: error.into(),
            cause: None,
        }
    }

    pub fn with_span(
        error: InternalErrorType,
        key: &'static str,
        span: InputSpan,
    ) -> Self {
        let mut spans = HashMap::new();
        spans.insert(key, span);
        InternalError {
            spans,
            error: error.into(),
            cause: None,
        }
    }

    pub fn with_spans(
        error: InternalErrorType,
        spans: HashMap<&'static str, InputSpan>,
    ) -> Self {
        InternalError {
            spans,
            error: error.into(),
            cause: None,
        }
    }

    pub fn span(
        mut self,
        key: &'static str,
        span: InputSpan,
    ) -> Self {
        self.spans.insert(key, span);
        self
    }

    pub fn cause(
        mut self,
        cause: InternalError,
    ) -> Self {
        if self.cause.is_some() {
            panic!("This error already has a cause")
        };
        self.cause = Some(Box::new(cause));
        self
    }

    fn get_span(
        &self,
        key: &'static str,
    ) -> InputSpan {
        self.spans
            .get(key)
            .expect(format!("key '{key}' should exist in error spans").as_str())
            .clone()
    }
}

impl InternalError {
    // TODO implement proper reports for PreprocessorError, InputCacheError and ProcessorSpecificationError and remove this function
    pub fn create_string(
        &self,
        mut input_cache: &mut CompilerInputCache,
    ) -> String {
        let mut str = Vec::new();
        match &self.error {
            InternalErrorType::ProcessorSpecificationError(e) => return e.to_string(),
            InternalErrorType::PreprocessorError(e) => return e.to_string(),
            _ => self
                .as_report()
                .write(&mut input_cache, &mut str)
                .expect("This cannot fail"),
        }
        String::from_utf8(str).expect("This cannot fail")
    }

    // TODO refactor this into separate functions
    pub fn as_report(&self) -> Report<'static, InputSpan> {
        (match &self.error {
            InternalErrorType::DuplicatedDefine(_) | InternalErrorType::DuplicatedSpaceAttribute(_) => {
                let first = self.get_span("first");
                Report::build(ReportKind::Error, first.clone())
                    .with_label(Label::new(first).with_message("first definition"))
                    .with_label(Label::new(self.get_span("second")).with_message("second definition"))
            }
            InternalErrorType::InstructionAfterExport => {
                let instr = self.get_span("instr");
                Report::build(ReportKind::Error, instr.clone())
                    .with_label(Label::new(self.get_span("export")).with_message("export is here"))
                    .with_label(Label::new(instr).with_message("instruction should be before export"))
            }
            InternalErrorType::InvalidFieldRange(invalid) => {
                let field = self.get_span("field");
                match invalid {
                    InvalidFieldRange::EndBeforeStart(range) => Report::build(ReportKind::Error, field.clone())
                        .with_label(
                            Label::new(field.clone())
                                .with_message(format!("range ends at {} but starts at {}", range.end, range.start)),
                        ),
                    InvalidFieldRange::TooLargeRange(bits, range) => Report::build(ReportKind::Error, field.clone())
                        .with_label(Label::new(field.clone()).with_message(format!(
                            "range fits {} bits, but there are only {} bits",
                            range.end - range.start,
                            bits
                        ))),
                }
            }
            InternalErrorType::SliceError(slice) => {
                let expr = self.get_span("expr");
                match slice {
                    SliceError::UndeterminedSize => Report::build(ReportKind::Error, expr.clone())
                        .with_label(Label::new(expr.clone()).with_message("size is undetermined")),
                    SliceError::InvalidSliceSize { .. } => Report::build(ReportKind::Error, expr.clone())
                        .with_label(Label::new(expr.clone()).with_message("invalid slice size")),
                    _ => todo!("add more slice error reports"),
                }
            }
            InternalErrorType::UndeclaredSymbol => {
                let ident = self.get_span("ident");
                Report::build(ReportKind::Error, ident.clone())
                    .with_label(Label::new(ident.clone()).with_message("Symbol is undeclared"))
            }
            InternalErrorType::UndefinedSymbol(symbol) => {
                let sym = self.get_span("symbol");
                Report::build(ReportKind::Error, sym.clone()).with_label(
                    Label::new(sym.clone()).with_message(format!("Symbol {:?} is undefined", symbol.symbol)),
                )
            }
            InternalErrorType::InvalidSymbolType(invalid) => {
                let mut colors = ColorGenerator::new();
                let color_a = colors.next();
                let color_b = colors.next();
                let ref_span = self.get_span("reference");
                let def_span = self.get_span("defined");
                Report::build(ReportKind::Error, ref_span.clone())
                    .with_message("Invalid symbol type.")
                    .with_label(
                        Label::new(def_span)
                            .with_message(format!("The symbol is defined here as {}", invalid.found.fg(color_a)))
                            .with_color(color_a),
                    )
                    .with_label(
                        Label::new(ref_span.clone())
                            .with_message(format!(
                                "The symbol referenced by {} has type {}, expected {}.",
                                invalid.symbol_reference.clone().fg(color_a),
                                invalid.found.fg(color_a),
                                invalid.expected.clone().fg(color_b)
                            ))
                            .with_color(color_a),
                    )
            }
            e => todo!("unhandled error {e:?} in spanned_report"),
        })
        .with_message(self.error.to_string())
        // TODO include causes recursively
        .finish()
    }
}

pub enum SleighCompilerError {
    SemanticError(String),
    ParserError(String),
    LexerError(String),
    Unknown,
    ErrorHandlingFailure,
}

impl From<ParseError> for SleighCompilerError {
    fn from(e: ParseError) -> Self {
        match e {
            ParseError::LexerError(msg) => SleighCompilerError::LexerError(msg),
            ParseError::InputCacheError(e) => SleighCompilerError::ParserError(e.to_string()),
            ParseError::ParserError(msg) => SleighCompilerError::ParserError(msg),
        }
    }
}

impl Debug for SleighCompilerError {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        match self {
            SleighCompilerError::SemanticError(s)
            | SleighCompilerError::ParserError(s)
            | SleighCompilerError::LexerError(s) => Display::fmt(s, f),
            SleighCompilerError::ErrorHandlingFailure => panic!("Error handling failed"),
            SleighCompilerError::Unknown => f.write_str("Unknown error occurred"),
        }
    }
}
