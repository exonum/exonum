// Copyright 2019 The Exonum Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// Workaround for `failure` see https://github.com/rust-lang-nursery/failure/issues/223 and
// ECR-1771 for the details.
#![allow(bare_trait_objects)]

//! The set of errors for the Exonum API module.

use actix_web::error::JsonPayloadError;
use failure::Fail;
use std::{fmt, io};

/// List of possible API errors.
#[derive(Fail, Debug)]
pub enum Error {
    /// Storage error. This type includes errors related to the database, caused
    /// by, for example, serialization issues.
    #[fail(display = "Storage error: {}", _0)]
    Storage(#[cause] failure::Error),

    /// Input/output error. This type includes errors related to files that are not
    /// a part of the Exonum storage.
    #[fail(display = "IO error: {}", _0)]
    Io(#[cause] io::Error),

    /// Bad request. This error occurs when the request contains invalid syntax.
    #[fail(display = "Bad request: {}", _0)]
    BadRequest(String),

    /// Not found. This error occurs when the server cannot locate the requested
    /// resource.
    #[fail(display = "Not found: {}", _0)]
    NotFound(String),

    /// Internal server error. This type can return any internal server error to the user.
    #[fail(display = "Internal server error: {}", _0)]
    InternalError(failure::Error),

    /// Unauthorized error. This error occurs when the request lacks valid
    /// authentication credentials.
    #[fail(display = "Unauthorized")]
    Unauthorized,

    /// Message length is exceeded.
    #[fail(
        display = "Payload too large: the allowed {}, while received {} bytes",
        _0, _1
    )]
    PayloadTooLarge {
        /// Variant of a limit for incoming requests.
        length_limit: LengthLimit,
        /// A length of content length.
        content_length: usize,
    },
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<failure::Error> for Error {
    fn from(e: failure::Error) -> Self {
        Error::InternalError(e)
    }
}

/// Length limit for incoming requests.
#[derive(Debug, Clone, Copy)]
pub enum LengthLimit {
    /// Limit for a message in bytes.
    Message(usize),
    /// Limit for Json body in bytes.
    Json(usize),
}

impl fmt::Display for LengthLimit {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self {
            LengthLimit::Message(len) => write!(f, "message limit is {} bytes", len),
            LengthLimit::Json(len) => write!(f, "json limit is {} bytes", len),
        }
    }
}

pub(crate) fn into_api_error(
    error: JsonPayloadError,
    length_limit: LengthLimit,
    content_length: String,
) -> Error {
    match error {
        JsonPayloadError::Overflow => Error::PayloadTooLarge {
            length_limit,
            content_length: content_length.parse().unwrap(),
        },
        JsonPayloadError::ContentType => Error::BadRequest("Wrong content type".to_owned()),
        JsonPayloadError::Deserialize(err) => {
            Error::BadRequest(format!("Json deserialize error: {}", err))
        }
        JsonPayloadError::Payload(err) => {
            Error::BadRequest(format!("Error while reading payload: {}", err))
        }
    }
}
