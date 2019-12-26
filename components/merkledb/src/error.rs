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

//! An implementation of `Error` type.

use failure::Fail;
use std::error::Error as StdError;

/// The error type for I/O operations with the `Database`.
///
/// Application code in most cases should consider these errors as fatal. At the same time,
/// it may be possible to recover from an error after manual intervention (e.g., by restarting
/// the process or freeing up more disc space).
#[derive(Fail, Debug, Clone)]
#[fail(display = "{}", message)]
pub struct Error {
    message: String,
}

impl Error {
    /// Creates a new storage error with an information message about the reason.
    pub fn new<T: Into<String>>(message: T) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl From<rocksdb::Error> for Error {
    fn from(err: rocksdb::Error) -> Self {
        Self::new(err.description())
    }
}
