use crate::compiler::InputId;
use crate::compiler::inputcache::InputCacheError;
use std::fmt::{
    Debug,
    Display,
    Formatter,
};
use thiserror::Error;

// TODO refactor PreprocessorError to use reports just as compiler errors
#[derive(Error)]
pub enum PreprocessorError {
    #[error("Failed to include a file specified by an @include directive.")]
    IncludeError(String),
    #[error("The input cache encountered an error")]
    InputCacheError(#[from] InputCacheError),
    #[error("Encountered a parsing error: {0}")]
    ParseError(String),
    #[error("The identifier is not defined")]
    UndefinedIdentifier(String),
    #[error("Absolute paths are not supported")]
    AbsolutePath,
    #[error("The included file needs to be loaded in order to continue")]
    LoadingRequired(InputId),
    #[error("No input was processed, please first parse some input, before preprocessing.")]
    NoInputProcessed,
}

impl Debug for PreprocessorError {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        match self {
            PreprocessorError::ParseError(e) => Display::fmt(e, f),
            PreprocessorError::IncludeError(e) => Display::fmt(e, f),
            PreprocessorError::InputCacheError(e) => Debug::fmt(e, f),
            PreprocessorError::UndefinedIdentifier(e) => Display::fmt(e, f),
            PreprocessorError::LoadingRequired(e) => Debug::fmt(e, f),
            _ => Display::fmt(self, f),
        }
    }
}
