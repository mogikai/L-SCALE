use crate::compiler::inputcache::InputCacheError;
use crate::compiler::lexer::InputSpan;
use crate::preprocessor::error::PreprocessorError;
use crate::spec::builder::{MOIRSymbolId, StringId};
use crate::spec::moir::common::MOIRSize;
use crate::spec::ProcessorSpecificationError;
use std::fmt::{
    Debug,
    Display,
    Formatter,
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
    CannotSliceUndeterminedSize,
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
    RangeTooLarge(u64, Range<u64>),
}

#[derive(Debug, Error)]
#[error("The constant value being constructed does not fit into the requested size. Value: {val:?}, Requested size: {size:?}"
)]
pub struct ConstDoesNotFit {
    pub val: u128,
    pub size: MOIRSize,
}

#[derive(Debug, Error)]
#[error("mask error")]
pub enum MaskError {
    ConstDoesNotFit(#[from] ConstDoesNotFit),
    SliceError(#[from] SliceError),
}

#[derive(Debug, Error)]
#[error("The symbol is already defined")]
pub struct SymbolAlreadyDefined {}

#[derive(Debug, Error)]
#[error("The argument count is not what it should be")]
pub struct InvalidArgumentCount {}

#[derive(Debug, Error)]
#[error("The identifier is not mapped")]
pub struct IdentifierNotMapped {
    pub field: StringId
}

#[derive(Debug, Error)]
#[error("The symbol was not declared before being defined")]
pub struct SymbolUndeclared {}

#[derive(Error, Debug)]
#[error("Cannot slice left hand side")]
pub struct CannotTruncateLhs {
    pub err: SliceError,
    pub ident_str: String,
    pub expected_size: MOIRSize,
}

#[derive(Error, Debug)]
#[error("Identifier already exists")]
pub struct IdentifierExists {
    pub name: String,
}

#[derive(Error, Debug)]
#[error("Definition of {dupe_type:?} already exists")]
pub struct DuplicateDefine {
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
pub struct DuplicateSpaceAttribute {
    pub attribute: SASTAddressSpaceAttribute,
}

#[derive(Error)]
pub enum SleighCompilerError {
    #[error("The preprocessor encountered an error.")]
    PreprocessorError(#[from] PreprocessorError),
    #[error("The input cache encountered an error.")]
    InputCacheError(#[from] InputCacheError),
    #[error("The processor specification encountered an error")]
    ProcessorSpecificationError(#[from] ProcessorSpecificationError),
    #[error("An error occurred during lexical analysis.")]
    LexerError(String),
    #[error("An error  occurred during parsing")]
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
    #[error("cannot create mask: {0}")]
    MaskError(#[from] MaskError),
    #[error("Cannot evaluate constants: {0}")]
    ConstEvalError(#[from] ConstEvalError),
    #[error("The field range is not valid: {0}")]
    InvalidFieldRange(#[from] InvalidFieldRange),
    #[error("Const does not fit: {0}")]
    ConstDoesNotFit(#[from] ConstDoesNotFit),
    #[error("Symbol is already defined {0}")]
    SymbolAlreadyDefined(#[from] SymbolAlreadyDefined),
    #[error("The builtin operation was called with an incorrect number of arguments: {0}")]
    InvalidArgumentCount(#[from] InvalidArgumentCount),
    #[error("Identifier not mapped: {0}")]
    IdentifierNotMapped(#[from] IdentifierNotMapped),
    #[error("Cannot truncate LHS: {0}")]
    CannotTruncateLhs(#[from] CannotTruncateLhs),
    #[error("Instruction after export")]
    InstructionAfterExport,
    #[error("Symbol undeclared: {0}")]
    SymbolUndeclared(#[from] SymbolUndeclared),
    #[error("Identifier exists: {0}")]
    IdentifierExists(#[from] IdentifierExists),
    #[error("Duplicate space attribute: {0}")]
    DuplicateSpaceAttribute(#[from] DuplicateSpaceAttribute),
    #[error("Duplicate define: {0}")]
    DuplicateDefine(#[from] DuplicateDefine),
    #[error("Undefined identifier: {0}")]
    UndefinedIdentifier(#[from] UndefinedIdentifier),
    #[error("Undefined symbol: {0}")]
    UndefinedSymbol(#[from] UndefinedSymbol),
    #[error("Invalid symbol type: {0}")]
    InvalidSymbolType(#[from] InvalidSymbolType),
    #[error("Spanned error: {0}")]
    SpannedError(SpannedError),
}

#[derive(Debug, Error)]
#[error("{spans:?} {error:?}")]
pub struct SpannedError {
    pub spans: Vec<InputSpan>,
    pub error: Box<SleighCompilerError>
}

impl SleighCompilerError {
    pub fn as_report(&self) -> Option<Report<'static, InputSpan>> {
        match self {
            SleighCompilerError::SpannedError(spanned_error) => {
                SleighCompilerError::spanned_report(&spanned_error.spans, &spanned_error.error)
            }
            _e => None, // TODO panic here
        }
    }

    fn spanned_report(spans: &Vec<InputSpan>, e: &SleighCompilerError) -> Option<Report<'static, InputSpan>> {
        let report = Report::build(ReportKind::Error, spans[0].context(), spans[0].start())
            .with_message(format!("{}", e));
        match e {
            SleighCompilerError::DuplicateDefine(_) | SleighCompilerError::DuplicateSpaceAttribute(_) => {
                Some(report
                    .with_label(Label::new(spans[0].clone()).with_message("first definition"))
                    .with_label(Label::new(spans[1].clone()).with_message("second definition"))
                    .finish())
            }
            SleighCompilerError::InstructionAfterExport => {
                Some(report
                    .with_label(Label::new(spans[0].clone()).with_message("export is here"))
                    .with_label(Label::new(spans[1].clone()).with_message("instruction should be before export"))
                    .finish())
            }
            SleighCompilerError::InvalidFieldRange(invalid) => {
                Some(match invalid {
                    InvalidFieldRange::EndBeforeStart(range) => {
                        report.with_label(Label::new(spans[0].clone())
                            .with_message(format!("range ends at {} but starts at {}", range.end, range.start)))
                            .finish()
                    }
                    InvalidFieldRange::RangeTooLarge(bits, range) => {
                        report.with_label(Label::new(spans[0].clone()).
                            with_message(format!("range fits {} bits, but there are only {} bits",
                                                 range.end - range.start, bits)))
                            .finish()
                    }
                })
            }
            SleighCompilerError::SliceError(slice) => {
                Some(match slice {
                    SliceError::CannotSliceUndeterminedSize => {
                        report
                            .with_label(Label::new(spans[0].clone()).with_message("size is undetermined"))
                            .finish()
                    }
                    _ => todo!("add more slice error reports")
                })
            }
            SleighCompilerError::SymbolUndeclared(_) => {
                Some(report.with_label(Label::new(spans[0].clone()).with_message("symbol is undeclared")).finish())

            }
            _ => None
        }
    }
}

impl Debug for SleighCompilerError {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        match self {
            SleighCompilerError::PreprocessorError(e) => Debug::fmt(e, f),
            SleighCompilerError::ProcessorSpecificationError(e) => Debug::fmt(e, f),
            SleighCompilerError::LexerError(e) => Display::fmt(e, f),
            SleighCompilerError::InputCacheError(e) => Debug::fmt(e, f),
            _ => Display::fmt(self, f),
        }
    }
}

#[macro_export]
macro_rules! span_err {
    ($e:expr,$span:expr) => {
        {
            use crate::compiler::error::SleighCompilerError;
            use crate::compiler::error::SpannedError;
            let e: SleighCompilerError = $e.into();
            SleighCompilerError::SpannedError(
                if let SleighCompilerError::SpannedError(mut spanned_error) = e {
                    spanned_error.spans.push($span);
                    spanned_error
                } else {
                    SpannedError {
                        spans: vec![$span], error: Box::new(e)
                    }
                }
            )
        }
    };
    ($e:expr, $($span:expr),+ $(,)?) => {
        {
            use crate::compiler::error::SleighCompilerError;
            use crate::compiler::error::SpannedError;
            let e: SleighCompilerError = $e.into();
            SleighCompilerError::SpannedError(
                if let SleighCompilerError::SpannedError(mut spanned_error) = e {
                    let mut vec = vec![$($span),+];
                    spanned_error.spans.append(&mut vec);
                    spanned_error
                } else {
                    SpannedError {
                        spans: vec![$($span),+], error: Box::new(e)
                    }
                }
            )
        }
    };
}
