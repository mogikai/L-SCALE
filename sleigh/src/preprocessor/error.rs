use std::io::Error;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PreprocessorError {
    #[error("Failed to include a file specified by an @include directive.")]
    IncludeError(#[from] Error),
    #[error("Encountered a parsing error")]
    ParseError(String),
    #[error("The identifier is not defined")]
    UndefinedIdentifier(String),
    #[error("Absolute paths are not supported")]
    AbsolutePath,
}
