use crate::errors::cpu::CPUError;
use crate::errors::filesystem::FSError;
use mogi_utilities::error::memory::MemoryError;
use std::error::Error;
use std::fmt::{
    Display,
    Formatter,
};

#[derive(Debug)]
pub enum GenericErrorKind {
    CPUError(CPUError),
    FSError(FSError),
    MemoryError(MemoryError),
    NotImplemented,
    Unknown,
}

#[derive(Debug)]
pub struct GenericError {
    kind: GenericErrorKind,
    message: String,
}

impl GenericError {
    pub fn new(
        kind: GenericErrorKind,
        message: &str,
    ) -> Self {
        Self {
            kind,
            message: message.to_string(),
        }
    }

    pub fn unknown() -> Self {
        Self {
            kind: GenericErrorKind::Unknown,
            message: "".to_string(),
        }
    }

    pub fn kind(&self) -> &GenericErrorKind {
        &self.kind
    }
}

impl Display for GenericError {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        write!(f, "GenericError({}): {}", self.kind, self.message)
    }
}

impl Display for GenericErrorKind {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Error for GenericError {}

impl From<FSError> for GenericError {
    fn from(e: FSError) -> Self {
        Self {
            kind: GenericErrorKind::FSError(e),
            message: "".to_string(),
        }
    }
}

impl From<CPUError> for GenericError {
    fn from(e: CPUError) -> Self {
        Self {
            kind: GenericErrorKind::CPUError(e),
            message: "".to_string(),
        }
    }
}

impl From<MemoryError> for GenericError {
    fn from(e: MemoryError) -> Self {
        Self {
            kind: GenericErrorKind::MemoryError(e),
            message: "".to_string(),
        }
    }
}
