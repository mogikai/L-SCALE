use crate::preprocessor::error::PreprocessorError;
use std::io::Error;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SleighCompilerError {
    #[error("Could not read the provided sleigh file.")]
    ReadError(#[from] Error),
    #[error("The preprocessor encountered an error.")]
    PreprocessorError(#[from] PreprocessorError),
}
