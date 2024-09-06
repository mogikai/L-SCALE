use crate::compiler::ast::Spanned;
use crate::compiler::inputcache::InputCacheError;
use crate::compiler::lexer::InputSpan;
use crate::preprocessor::error::PreprocessorError;
use crate::spec::builder::StringId;
use crate::spec::moir::common::MOIRSize;
use crate::spec::ProcessorSpecificationError;
use ariadne::Report;
use std::fmt::{
    Debug,
    Display,
    Formatter,
};
use thiserror::Error;

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
    #[error("The compiler encountered an error during the semantic analysis phase.")]
    SemanticError(String),
    #[error("The value cannot be converted into a boolean value")]
    CannotConverToBool,
    #[error("The operation encountered an overflow during constant folding")]
    ConstEvalOverflow,
    #[error("A detailed error report referencing the inputs.")]
    ErrorReport(Box<Report<'static, InputSpan>>),
    #[error("The identifier is not mapped.")]
    IdentifierNotMapped(Spanned<StringId>),
    #[error("The symbol was not declared before being defined.")]
    SymbolUndeclared,
    #[error("The value cannot be sliced, since the requested slice is either too large or too small. New size: {new:?}, current size: {current:?}")]
    InvalidSliceSize { new: MOIRSize, current: MOIRSize },
    #[error("The value cannot be sliced, since the slice range is invalid")]
    InvalidSliceRange {
        current_start: MOIRSize,
        current_end: MOIRSize,
        new_start: MOIRSize,
        new_end: MOIRSize,
    },
    #[error("This error needs replacement")]
    TODO,
    #[error("The slice cannot be sliced as long as the end is not determined")]
    CannotSliceUndetermined,
    #[error("The compiler was configured to stop on the first error encountered")]
    StopOnFirstError,
    #[error("The constant value being constructed does not fit into the requested size. Value: {val:?}, Requested size: {size:?}")]
    ConstDoesNotFit { val: u128, size: MOIRSize },
    #[error("The symbol is already defined")]
    DuplicateDefinition,
}

impl Debug for SleighCompilerError {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        match self {
            SleighCompilerError::PreprocessorError(e) => Debug::fmt(e, f),
            SleighCompilerError::LexerError(e) => Display::fmt(e, f),
            SleighCompilerError::InputCacheError(e) => Debug::fmt(e, f),
            SleighCompilerError::SemanticError(e) => Display::fmt(e, f),
            _ => Display::fmt(self, f),
        }
    }
}
