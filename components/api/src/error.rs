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

pub use actix_web::http::{
    header::{self, HeaderName},
    HeaderMap, StatusCode as HttpStatusCode,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use std::fmt;

/// API HTTP error struct.
#[derive(Error, Debug)]
#[non_exhaustive]
pub struct Error {
    /// HTTP error code.
    pub http_code: HttpStatusCode,
    /// API error body.
    pub body: ErrorBody,
    /// Additional HTTP headers.
    pub headers: HeaderMap,
}

impl Default for Error {
    fn default() -> Self {
        Self {
            http_code: HttpStatusCode::default(),
            body: ErrorBody::default(),
            headers: HeaderMap::new(),
        }
    }
}

/// Body of an `Error` serialized into the body of erroneous server responses.
#[derive(Debug, Serialize, Deserialize, Default)]
#[non_exhaustive]
pub struct ErrorBody {
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

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.body.title, self.body.detail)
    }
}

impl Error {
    /// Builds a ApiError with the given `http_code`.
    pub fn new(http_code: HttpStatusCode) -> Self {
        Self {
            http_code,
            body: ErrorBody::default(),
            headers: HeaderMap::new(),
        }
    }

    /// Builds Bad Request (400) error.
    pub fn bad_request() -> Self {
        Error::new(HttpStatusCode::BAD_REQUEST)
    }

    /// Builds Forbidden (403) error.
    pub fn forbidden() -> Self {
        Error::new(HttpStatusCode::FORBIDDEN)
    }

    /// Builds Not Found (404) error.
    pub fn not_found() -> Self {
        Error::new(HttpStatusCode::NOT_FOUND)
    }

    /// Builds Internal Server Error (500).
    pub fn internal(cause: impl fmt::Display) -> Self {
        Error::new(HttpStatusCode::INTERNAL_SERVER_ERROR).detail(cause.to_string())
    }

    /// Sets `docs_uri` of an error.
    pub fn docs_uri(mut self, docs_uri: impl Into<String>) -> Self {
        self.body.docs_uri = docs_uri.into();
        self
    }

    /// Sets `title` of an error.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.body.title = title.into();
        self
    }

    /// Sets `detail` of an error.
    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.body.detail = detail.into();
        self
    }

    /// Sets `source` of an error.
    #[doc(hidden)]
    pub fn source(mut self, source: impl Into<String>) -> Self {
        self.body.source = source.into();
        self
    }

    /// Sets `error_code` of an error.
    pub fn error_code(mut self, error_code: u8) -> Self {
        self.body.error_code = Some(error_code);
        self
    }

    /// Adds HTTP header, which will be added in `HttpResponse`
    pub(crate) fn header(mut self, key: HeaderName, value: &str) -> Self {
        self.headers.insert(key, value.parse().unwrap());
        self
    }

    /// Tries to create `ApiError` from JSON.
    pub fn parse(
        http_code: HttpStatusCode,
        body: &str,
    ) -> std::result::Result<Self, serde_json::Error> {
        let body = if !body.is_empty() {
            serde_json::from_str(body)?
        } else {
            ErrorBody::default()
        };

        Ok(Self {
            http_code,
            body,
            headers: HeaderMap::new(),
        })
    }
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

        Error::new(HttpStatusCode::MOVED_PERMANENTLY).header(header::LOCATION, &full_location)
    }
}
