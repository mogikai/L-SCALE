use crate::preprocessor::error::PreprocessorError;
use std::fmt::{
    Debug,
    Display,
    Formatter,
};
use std::io::Error;
use thiserror::Error;

#[derive(Error)]
pub enum SleighCompilerError {
    #[error("Could not read the provided sleigh file.")]
    ReadError(#[from] Error),
    #[error("The preprocessor encountered an error.")]
    PreprocessorError(#[from] PreprocessorError),
    #[error("An error occurred during lexical analysis.")]
    LexerError(String),
    #[error("An error  occurred during parsing")]
    ParserError(String),
    #[error("The requested source was not found.")]
    MissingSource,
}

impl Debug for SleighCompilerError {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        match self {
            SleighCompilerError::ReadError(e) => Debug::fmt(e, f),
            SleighCompilerError::PreprocessorError(e) => Debug::fmt(e, f),
            SleighCompilerError::LexerError(e) => Display::fmt(e, f),
            _ => Display::fmt(self, f),
        }
    }
}
