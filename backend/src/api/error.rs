use std::fmt;
use std::error;
use std::io::Error as IoError;
use std::collections::BTreeMap;

use serde_json::value::ToJson;
use iron::prelude::*;
use iron::IronError;
use iron::status;

use exonum::crypto::{HexValue, FromHexError, Hash};
use exonum::storage::Error as StorageError;
use exonum::events::Error as EventsError;

#[derive(Debug)]
pub enum Error {
    Storage(StorageError),
    Events(EventsError),
    FromHex(FromHexError),
    Io(IoError),
    FileNotFound(Hash),
    FileToBig,
    FileExists(Hash),
    IncorrectRequest,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        match *self {
            Error::Storage(_) => "Storage",
            Error::Events(_) => "Events",
            Error::FromHex(_) => "FromHex",
            Error::Io(_) => "Io",
            Error::FileNotFound(_) => "FileNotFound",
            Error::FileToBig => "FileToBig",
            Error::FileExists(_) => "FileExists",
            Error::IncorrectRequest => "IncorrectRequest",
        }
    }
}

impl From<IoError> for Error {
    fn from(e: IoError) -> Error {
        Error::Io(e)
    }
}

impl From<StorageError> for Error {
    fn from(e: StorageError) -> Error {
        Error::Storage(e)
    }
}

impl From<EventsError> for Error {
    fn from(e: EventsError) -> Error {
        Error::Events(e)
    }
}

impl From<FromHexError> for Error {
    fn from(e: FromHexError) -> Error {
        Error::FromHex(e)
    }
}

impl From<Error> for IronError {
    fn from(e: Error) -> IronError {
        use std::error::Error as StdError;

        let mut body = BTreeMap::new();
        body.insert("type", e.description().into());
        let code = match e {
            Error::FileExists(hash) => {
                body.insert("hash", hash.to_hex());
                status::Conflict
            }
            Error::FileNotFound(hash) => {
                body.insert("hash", hash.to_hex());
                status::Conflict
            }
            _ => status::Conflict,
        };
        IronError {
            error: Box::new(e),
            response: Response::with((code, body.to_json().to_string())),
        }
    }
}