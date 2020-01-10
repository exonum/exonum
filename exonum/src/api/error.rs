// Copyright 2020 The Exonum Team
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

use failure::Fail;
use serde::Serialize;

use std::collections::HashMap;
use std::io;

use crate::node::SendError;

#[derive(Debug, Serialize)]
pub enum HttpCode {
    Unexpected,
    BadRequest,
    NotFound,
}

#[derive(Fail, Debug, Serialize)]
pub struct ApiError {
    pub http_code: HttpCode,
    pub error_type: String,
    pub title: String,
    pub detail: String,
    pub params: HashMap<String, String>,
    pub source: String,
    pub error_code: i8,
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&serde_json::to_string(self).unwrap())
    }
}

impl ApiError {
    fn default() -> Self {
        Self {
            http_code: HttpCode::Unexpected,
            error_type: String::new(),
            title: String::new(),
            detail: String::new(),
            params: HashMap::new(),
            source: String::new(),
            error_code: 0,
        }
    }

    pub fn BadRequest() -> Self {
        Self {
            http_code: HttpCode::BadRequest,
            ..Self::default()
        }
    }

    pub fn NotFound() -> Self {
        Self {
            http_code: HttpCode::NotFound,
            ..Self::default()
        }
    }

    pub fn error_type(mut self, error_type: String) -> Self {
        self.error_type = error_type;
        self
    }

    pub fn title(mut self, title: String) -> Self {
        self.title = title;
        self
    }

    pub fn detail(mut self, detail: String) -> Self {
        self.detail = detail;
        self
    }

    pub fn param(mut self, key: String, value: String) -> Self {
        self.params.insert(key, value);
        self
    }

    pub fn source(mut self, source: String) -> Self {
        self.source = source;
        self
    }

    pub fn error_code(mut self, error_code: i8) -> Self {
        self.error_code = error_code;
        self
    }
}

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
pub struct MovedPermanentlyError {
    location: String,
    query_part: Option<String>,
}

impl MovedPermanentlyError {
    /// Creates a new builder object with base url.
    /// Note that query parameters should **not** be added to the location url,
    /// for this purpose use `with_query` method.
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
}

impl From<MovedPermanentlyError> for Error {
    fn from(e: MovedPermanentlyError) -> Self {
        let full_location = match e.query_part {
            Some(query) => format!("{}?{}", e.location, query),
            None => e.location,
        };

        Error::MovedPermanently(full_location)
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}

/// Converts the provided error into an internal server error.
impl From<failure::Error> for Error {
    fn from(e: failure::Error) -> Self {
        Error::InternalError(e)
    }
}

/// Converts the provided error into an internal server error.
impl From<SendError> for Error {
    fn from(e: SendError) -> Self {
        Error::InternalError(e.into())
    }
}
