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

// These functions transform source error types into other.
#![cfg_attr(feature = "cargo-clippy", allow(clippy::needless_pass_by_value))]

use failure::Error;
use log::error;

use std::{error::Error as StdError, fmt::Display};

pub fn log_error<E: Display>(error: E) {
    error!("An error occurred: {}", error)
}

pub trait LogError {
    fn log_error(self);
}

pub fn into_failure<E: StdError + Sync + Send + 'static>(error: E) -> Error {
    Error::from_boxed_compat(Box::new(error))
}

impl<T, E> LogError for Result<T, E>
where
    E: Display,
{
    fn log_error(self) {
        if let Err(error) = self {
            error!("An error occurred: {}", error);
        }
    }
}
