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

//! The set of errors for the Exonum API module.

use serde::Serialize;
use std::io;

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

    /// Moved permanently. This error means that resource existed at the specified
    /// location, but now is moved to the other place.
    #[fail(display = "Moved permanently; Location: {}", _0)]
    MovedPermanently(String),

    /// Gone. This error means that resource existed in the past, but now is not present.
    #[fail(display = "Gone")]
    Gone,

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
}

/// A helper structure allowing to build `MovedPermanently` response from the
/// composite parts.
#[derive(Debug)]
pub struct MovedPermanentlyErrorBuilder {
    location: String,
    query_part: Option<String>,
}

impl MovedPermanentlyErrorBuilder {
    /// Creates a new builder object with base url.
    pub fn new(location: String) -> Self {
        Self {
            location,
            query_part: None,
        }
    }

    /// Appends a query to the url.
    pub fn with_query<Q: Serialize>(self, query: Q) -> Self {
        let serialized_query =
            serde_urlencoded::to_string(query).expect("Unable to serialize query.");
        Self {
            query_part: Some(serialized_query),
            ..self
        }
    }

    /// Finalizes the building process.
    pub fn build(self) -> Error {
        let full_location = match self.query_part {
            Some(query) => format!("{}?{}", self.location, query),
            None => self.location,
        };

        Error::MovedPermanently(full_location)
    }
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
