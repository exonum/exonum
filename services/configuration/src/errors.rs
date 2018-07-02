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

use exonum::{
    blockchain::{ExecutionError, StoredConfiguration}, crypto::Hash,
    encoding::serialize::json::reexport::Error as JsonError, helpers::Height,
};

use transactions::Propose;

/// Error codes emitted by `Propose` and/or `Vote` transactions during execution.
#[derive(Debug)]
#[repr(u8)]
pub enum ErrorCode {
    /// Next configuration is already scheduled.
    ///
    /// Can be emitted by `Propose` or `Vote`.
    AlreadyScheduled = 0,
    /// The sender of the transaction is not among the active validators.
    ///
    /// Can be emitted by `Propose` or `Vote`.
    UnknownSender = 1,
    /// The configuration in the proposal does not reference the currently active configuration.
    ///
    /// Can be emitted by `Propose` or `Vote`.
    InvalidConfigRef = 2,
    /// Current blockchain height exceeds the height of the proposal activation.
    ///
    /// Can be emitted by `Propose` or `Vote`.
    ActivationInPast = 3,

    /// The same configuration is already proposed.
    ///
    /// Specific for `Propose`.
    AlreadyProposed = 32,
    /// The configuration in the transaction cannot be parsed.
    ///
    /// Specific for `Propose`.
    InvalidConfig = 33,

    /// The configuration has invalid majority_count.
    ///
    /// Specific for `Propose`.
    InvalidMajorityCount = 34,

    /// The transaction references an unknown configuration.
    ///
    /// Specific for `Vote`.
    UnknownConfigRef = 64,
    /// The validator who authored the transaction has already voted for the same proposal.
    ///
    /// Specific for `Vote`.
    AlreadyVoted = 65,
}

// Common error types for `Propose` and `Vote`.
#[derive(Debug, Fail)]
pub(crate) enum Error {
    #[fail(display = "Next configuration is already scheduled: {:?}", _0)]
    AlreadyScheduled(StoredConfiguration),

    #[fail(display = "Not authored by a validator")]
    UnknownSender,

    #[fail(display = "Does not reference actual config {:?}", _0)]
    InvalidConfigRef(StoredConfiguration),

    #[fail(display = "Current height {:?} greater or equal than `actual_from`", _0)]
    ActivationInPast(Height),

    #[fail(display = "Already proposed; old proposal: {:?}", _0)]
    AlreadyProposed(Propose),

    #[fail(display = "Cannot parse configuration: {}", _0)]
    InvalidConfig(#[cause] JsonError),

    #[fail(
        display = "Invalid majority count: {}, it should be >= {} and <= {}", proposed, min, max
    )]
    InvalidMajorityCount {
        min: usize,
        max: usize,
        proposed: usize,
    },

    #[fail(display = "Does not reference known config with hash {:?}", _0)]
    UnknownConfigRef(Hash),

    #[fail(display = "Validator already voted for a referenced proposal")]
    AlreadyVoted,
}

impl Error {
    fn code(&self) -> ErrorCode {
        use self::Error::*;

        match *self {
            AlreadyScheduled(..) => ErrorCode::AlreadyScheduled,
            UnknownSender => ErrorCode::UnknownSender,
            InvalidConfigRef(..) => ErrorCode::InvalidConfigRef,
            ActivationInPast(..) => ErrorCode::ActivationInPast,
            AlreadyProposed(..) => ErrorCode::AlreadyProposed,
            InvalidConfig(..) => ErrorCode::InvalidConfig,
            InvalidMajorityCount { .. } => ErrorCode::InvalidMajorityCount,
            UnknownConfigRef(..) => ErrorCode::UnknownConfigRef,
            AlreadyVoted => ErrorCode::AlreadyVoted,
        }
    }
}

impl From<Error> for ExecutionError {
    fn from(value: Error) -> ExecutionError {
        ExecutionError::with_description(value.code() as u8, value.to_string())
    }
}
