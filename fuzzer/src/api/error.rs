use thiserror::Error;

#[derive(Error, Debug)]
pub enum FuzzerError {
    #[error("An unknown error occurred")]
    Unknown,
}
