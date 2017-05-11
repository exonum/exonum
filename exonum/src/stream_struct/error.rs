use std::error::Error as StdError;
use std::fmt;
use super::Offset;

#[derive( Debug)]
pub enum Error {
    UnexpectedlyShortPayload { actual_size: u32, minimum_size: u32 },
    IncorrectBoolean { position: Offset, value: u8 },
    IncorrectSegmentReference { position: Offset, value: u32 },
    IncorrectSegmentSize { position: Offset, value: u32 },
    UnexpectedlyShortRawMessage { position: Offset, size: u32 },
    IncorrectSizeOfRawMessage { position: Offset, actual_size: u32, declared_size: u32 },
    IncorrectMessageType { position: Offset, actual_message_type: u16, declared_message_type: u16 },
    Utf8 {
        position: Offset,
        error: ::std::str::Utf8Error,
    },
    Other (Box<StdError>),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} = {:?}", self.description(), self)
    }
}

impl StdError for Error {
    fn description(&self) -> &str {
        match *self {
            Error::UnexpectedlyShortPayload { .. } => "Unexpectedly short payload", 
            Error::IncorrectBoolean { .. } => "Incorrect bool.",
            Error::IncorrectSegmentReference { .. } => "Incorrect segment reference.",
            Error::IncorrectSegmentSize { .. } => "Incorrect segment size.",
            Error::UnexpectedlyShortRawMessage { .. } => "Unexpectedly short RawMessage.",
            Error::IncorrectSizeOfRawMessage { .. } => "Incorrect size of RawMessage.",
            Error::IncorrectMessageType { .. } => "Incorrect message type.",
            Error::Utf8 { .. } => "Utf8 error in parsing string.",
            Error::Other(_) => "Other error.",
        }
    }
}

impl From<Box<StdError>> for Error {
    fn from(t: Box<StdError>) -> Error {
        Error::Other(t)
    }
}