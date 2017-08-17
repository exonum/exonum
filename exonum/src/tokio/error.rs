// Copyright 2017 The Exonum Team
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

use std::error::Error as StdError;
use std::io;

// Common error helpers

pub fn other_error<S: AsRef<str>>(s: S) -> io::Error {
    io::Error::new(io::ErrorKind::Other, s.as_ref())
}

pub fn forget_result<T>(_: T) {}

pub fn result_ok<T, E: StdError>(_: T) -> Result<(), E> {
    Ok(())
}

pub fn log_error<E: StdError>(err: E) {
    error!("An error occured: {}", err)
}

pub fn into_other<E: StdError>(err: E) -> io::Error {
    other_error(&format!("An error occured, {}", err.description()))
}
