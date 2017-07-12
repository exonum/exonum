//! An implementation of `Error` type.
use std::fmt;
use std::error;

/// The error type for I/O operations with storage.
#[derive(Debug, Clone)]
pub struct Error {
    message: String,
}

impl Error {
    /// Creates a new storage error with an information message about the reason.
    pub fn new<T: Into<String>>(message: T) -> Error {
        Error { message: message.into() }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Storage error: {}", self.message)
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        &self.message
    }
}
