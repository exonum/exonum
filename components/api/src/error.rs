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

use actix_web::http::StatusCode;
use failure::Fail;
use serde::Serialize;

use std::io;

/// API HTTP error struct.
#[derive(Fail, Debug, Serialize)]
pub struct ApiError {
    /// HTTP error code.
    #[serde(skip)]
    pub http_code: StatusCode,
    /// A URI reference to the documentation or possible solutions for the problem.
    #[serde(rename = "type", default, skip_serializing_if = "String::is_empty")]
    pub docs_uri: String,
    /// Short description of the error.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub title: String,
    /// Detailed description of the error.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub detail: String,
    /// Source of the error.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source: String,
    /// Internal error code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_code: Option<u8>,
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&serde_json::to_string(self).unwrap())
    }
}

impl ApiError {
    fn default() -> Self {
        Self {
            http_code: StatusCode::NOT_IMPLEMENTED,
            docs_uri: String::new(),
            title: String::new(),
            detail: String::new(),
            source: String::new(),
            error_code: None,
        }
    }

    /// Builds a BadRequest error.
    #[allow(non_snake_case)]
    pub fn BadRequest() -> Self {
        Self {
            http_code: StatusCode::BAD_REQUEST,
            ..Self::default()
        }
    }

    /// Builds a NotFound error.
    #[allow(non_snake_case)]
    pub fn NotFound() -> Self {
        Self {
            http_code: StatusCode::NOT_FOUND,
            ..Self::default()
        }
    }

    /// Sets `docs_uri` of an error.
    pub fn docs_uri(mut self, docs_uri: impl Into<String>) -> Self {
        self.docs_uri = docs_uri.into();
        self
    }

    /// Sets `title` of an error.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// Sets `detail` of an error.
    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = detail.into();
        self
    }

    /// Sets `source` of an error.
    pub fn source(mut self, source: impl Into<String>) -> Self {
        self.source = source.into();
        self
    }

    /// Sets `error_code` of an error.
    pub fn error_code(mut self, error_code: u8) -> Self {
        self.error_code = Some(error_code);
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
