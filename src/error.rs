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

/// The error type for I/O operations with storage.
///
/// These errors result in a panic. Storage errors are fatal as in the case of
/// database issues, the system stops working. Assuming that there are other
/// nodes and secret keys and other crucial data are not stored in the data base,
/// the operation of the system can be resumed from a backup or by rebooting the node.
#[derive(Fail, Debug, Clone)]
#[fail(display = "{}", message)]
pub struct Error {
    message: String,
}

impl Error {
    /// Creates a new storage error with an information message about the reason.
    pub(crate) fn new<T: Into<String>>(message: T) -> Self {
        Self {
            message: message.into(),
        }
    }
}
