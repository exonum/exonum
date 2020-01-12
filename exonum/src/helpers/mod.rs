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

//! Different assorted utilities.

pub use self::types::{Height, Round, ValidatorId};

// Required by `consensus-tests`. This is not a public API, since `user_agent::get` is hidden
// under `doc(hidden)`.
pub use self::user_agent::user_agent;

pub(crate) use self::ordered_map::OrderedMap;
// `Milliseconds` is just `u64`, but more readable within context.
pub use self::types::Milliseconds;

mod ordered_map;

use env_logger::Builder;
use exonum_merkledb::Fork;
use log::SetLoggerError;

use crate::blockchain::Schema;

mod types;
mod user_agent;

/// Performs the logger initialization.
pub fn init_logger() -> Result<(), SetLoggerError> {
    Builder::from_default_env()
        .default_format_timestamp_nanos(true)
        .try_init()
}

/// Basic trait to validate user defined input.
pub trait ValidateInput: Sized {
    /// The type returned in the event of a validate error.
    type Error;
    /// Perform parameters validation for this configuration and return error if
    /// value is inconsistent.
    fn validate(&self) -> Result<(), Self::Error>;
    /// The same as validate method, but returns the value itself as a successful result.
    fn into_validated(self) -> Result<Self, Self::Error> {
        self.validate().map(|_| self)
    }
}

/// Clears consensus messages cache.
///
/// Used in `exonum-cli` to implement `clear-cache` maintenance action.
#[doc(hidden)]
pub fn clear_consensus_messages_cache(fork: &Fork) {
    Schema::new(fork).consensus_messages_cache().clear();
}

/// Returns sufficient number of votes for the given validators number.
pub fn byzantine_quorum(total: usize) -> usize {
    total * 2 / 3 + 1
}
