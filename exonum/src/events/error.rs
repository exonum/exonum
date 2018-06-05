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

// These functions transform source error types into other.
#![cfg_attr(feature="cargo-clippy", allow(needless_pass_by_value))]

use std::{error::Error as StdError, io};

// Common error helpers (TODO move to helpers)

pub fn other_error<S: AsRef<str>>(s: S) -> io::Error {
    io::Error::new(io::ErrorKind::Other, s.as_ref())
}

pub fn result_ok<T, E: StdError>(_: T) -> Result<(), E> {
    Ok(())
}

pub fn log_error<E: StdError>(err: E) {
    error!("An error occurred: {}", err)
}

pub fn into_other<E: StdError>(err: E) -> io::Error {
    other_error(&format!("An error occurred, {}", err.description()))
}

pub trait LogError {
    fn log_error(self);
}

impl<T, E> LogError for Result<T, E>
where
    E: ::std::fmt::Display,
{
    fn log_error(self) {
        if let Err(error) = self {
            error!("An error occurred: {}", error);
        }
    }
}
