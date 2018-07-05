// Copyright 2018 The Exonum Team
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

//! Error module.

use std::{error, io};

use storage;

/// List of possible API errors.
#[derive(Fail, Debug)]
pub enum Error {
    /// Storage error.
    #[fail(display = "Storage error: {}", _0)]
    Storage(#[cause] storage::Error),

    /// Input/output error.
    #[fail(display = "IO error: {}", _0)]
    Io(#[cause] io::Error),

    /// Bad request.
    #[fail(display = "Bad request: {}", _0)]
    BadRequest(String),

    /// Not found.
    #[fail(display = "Not found: {}", _0)]
    NotFound(String),

    /// Internal error.
    #[fail(display = "Internal server error: {}", _0)]
    InternalError(Box<error::Error + Send + Sync>),

    /// Unauthorized error.
    #[fail(display = "Unauthorized")]
    Unauthorized,
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<storage::Error> for Error {
    fn from(e: storage::Error) -> Self {
        Error::Storage(e)
    }
}
