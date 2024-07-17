use crate::compiler::InputCacheError;
use std::fmt::{
    Debug,
    Display,
    Formatter,
    Pointer,
};
use std::io::Error;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error)]
pub enum PreprocessorError {
    #[error("Failed to include a file specified by an @include directive.")]
    IncludeError(#[from] Error),
    #[error("The input cache encountered an error")]
    InputCacheError(#[from] InputCacheError),
    #[error("Encountered a parsing error")]
    ParseError(String),
    #[error("The identifier is not defined")]
    UndefinedIdentifier(String),
    #[error("Absolute paths are not supported")]
    AbsolutePath,
    #[error("The included file needs to be loaded in order to continue")]
    LoadingRequired(PathBuf),
}

impl Debug for PreprocessorError {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        match self {
            PreprocessorError::ParseError(e) => Display::fmt(e, f),
            PreprocessorError::IncludeError(e) => Debug::fmt(e, f),
            PreprocessorError::InputCacheError(e) => Debug::fmt(e, f),
            PreprocessorError::UndefinedIdentifier(e) => f.write_fmt(format_args!("Undefined identifier {e:?}")),
            PreprocessorError::LoadingRequired(e) => Debug::fmt(e, f),
            _ => Display::fmt(self, f),
        }
    }
}
