use std::fmt::{
    Debug,
    Display,
    Formatter,
};

use thiserror::Error;

use crate::compiler::inputcache::InputCacheError;
use crate::preprocessor::error::PreprocessorError;
use crate::spec::ProcessorSpecificationError;

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
