use std::io;
use tracing::trace;

#[derive(Debug, Copy, Clone, PartialEq)]
#[non_exhaustive]
pub enum FSError {
    NotFound,
    NoEntry,
    Not4kAlligned,
    NoAddress,
    IO,
    Invalid,
    Loop,
    Unknown,
    Exist,
    Busy,
    NotEmpty,
    NotDir,
}

impl From<io::Error> for FSError {
    fn from(e: io::Error) -> Self {
        trace!("{}", e);
        FSError::IO
    }
}
