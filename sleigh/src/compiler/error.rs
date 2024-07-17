use crate::compiler::InputCacheError;
use crate::preprocessor::error::PreprocessorError;
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
    #[error("An error occurred during lexical analysis.")]
    LexerError(String),
    #[error("An error  occurred during parsing")]
    ParserError(String),
    #[error("The requested source was not found.")]
    MissingSource,
    #[error("The input is not yet tokenized.")]
    NotTokenized,
}

impl Debug for SleighCompilerError {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        match self {
            SleighCompilerError::PreprocessorError(e) => Debug::fmt(e, f),
            SleighCompilerError::LexerError(e) => Display::fmt(e, f),
            _ => Display::fmt(self, f),
        }
    }
}
