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

use exonum::blockchain::{ExecutionError, StoredConfiguration};
use exonum::crypto::Hash;
use exonum::encoding::serialize::json::reexport::Error as JsonError;
use exonum::helpers::Height;

use transactions::Propose;

// Implements conversion between two field-less enums.
macro_rules! convert_codes {
    ($from:ident => $to:ident { $($variant:ident,)+ }) => {
        #[doc(hidden)]
        impl From<$from> for $to {
            fn from(value: $from) -> $to {
                match value {
                    $($from::$variant => $to::$variant,)+
                }
            }
        }
    }
}

#[derive(Debug)]
#[repr(u8)]
enum CommonErrorCode {
    AlreadyScheduled = 0,
    UnknownSender,
    InvalidConfigRef,
    ActivationInPast,
}

/// Error codes emitted by [`Propose`] transactions during execution.
///
/// [`Propose`]: struct.Propose.html
#[derive(Debug)]
#[repr(u8)]
pub enum ProposeErrorCode {
    /// Next configuration is already scheduled.
    AlreadyScheduled = 0,
    /// The sender of the transaction is not among the active validators.
    UnknownSender,
    /// The configuration in the proposal does not reference the currently active configuration.
    InvalidConfigRef,
    /// Current blockchain height exceeds the height of the proposal activation.
    ActivationInPast,
    /// The same configuration is already proposed.
    AlreadyProposed = 32,
    /// The configuration in the transaction cannot be parsed.
    UnparseableConfig,
}

/// Error codes emitted by [`Vote`] transactions during execution.
///
/// [`Vote`]: struct.Vote.html
#[derive(Debug)]
#[repr(u8)]
pub enum VoteErrorCode {
    /// Next configuration is already scheduled.
    AlreadyScheduled = 0,
    /// The sender of the transaction is not among the active validators.
    UnknownSender,
    /// The configuration in the proposal does not reference the currently active configuration.
    InvalidConfigRef,
    /// Current blockchain height exceeds the height of the proposal activation.
    ActivationInPast,
    /// The transaction references an unknown configuration.
    UnknownConfigRef = 32,
    /// The validator who authored the transaction has already voted for the same proposal.
    AlreadyVoted,
}

convert_codes!(CommonErrorCode => ProposeErrorCode {
    AlreadyScheduled,
    UnknownSender,
    InvalidConfigRef,
    ActivationInPast,
});

convert_codes!(CommonErrorCode => VoteErrorCode {
    AlreadyScheduled,
    UnknownSender,
    InvalidConfigRef,
    ActivationInPast,
});

// Common error types for `Propose` and `Vote`.
#[derive(Debug, Fail)]
pub(crate) enum CommonError {
    #[fail(display = "Next configuration is already scheduled: {:?}", _0)]
    AlreadyScheduled(StoredConfiguration),

    #[fail(display = "Not authored by a validator")]
    UnknownSender,

    #[fail(display = "Does not reference actual config {:?}", _0)]
    InvalidConfigRef(StoredConfiguration),

    #[fail(display = "Current height {:?} greater or equal than `actual_from`", _0)]
    ActivationInPast(Height),
}

impl CommonError {
    fn code(&self) -> CommonErrorCode {
        use self::CommonError::*;

        match *self {
            AlreadyScheduled(..) => CommonErrorCode::AlreadyScheduled,
            UnknownSender => CommonErrorCode::UnknownSender,
            InvalidConfigRef(..) => CommonErrorCode::InvalidConfigRef,
            ActivationInPast(..) => CommonErrorCode::ActivationInPast,
        }
    }
}

#[derive(Debug, Fail)]
pub(crate) enum ProposeError {
    #[fail(display = "{}", _0)]
    Common(
        #[cause]
        CommonError
    ),

    #[fail(display = "Already proposed; old proposal: {:?}", _0)]
    AlreadyProposed(Propose),

    #[fail(display = "Cannot parse configuration: {}", _0)]
    UnparseableConfig(
        #[cause]
        JsonError
    ),
}

impl ProposeError {
    fn code(&self) -> ProposeErrorCode {
        use self::ProposeError::*;

        match *self {
            Common(ref err) => err.code().into(),
            AlreadyProposed(..) => ProposeErrorCode::AlreadyProposed,
            UnparseableConfig(..) => ProposeErrorCode::UnparseableConfig,
        }
    }
}

impl From<CommonError> for ProposeError {
    fn from(value: CommonError) -> ProposeError {
        ProposeError::Common(value)
    }
}

impl From<ProposeError> for ExecutionError {
    fn from(value: ProposeError) -> ExecutionError {
        ExecutionError::new(value.code() as u8)
    }
}

#[derive(Debug, Fail)]
pub(crate) enum VoteError {
    #[fail(display = "{}", _0)]
    Common(
        #[cause]
        CommonError
    ),

    #[fail(display = "Does not reference known config with hash {:?}", _0)]
    UnknownConfigRef(Hash),

    #[fail(display = "Validator already voted for a referenced proposal")]
    AlreadyVoted,
}

impl VoteError {
    fn code(&self) -> VoteErrorCode {
        use self::VoteError::*;

        match *self {
            Common(ref err) => err.code().into(),
            UnknownConfigRef(..) => VoteErrorCode::UnknownConfigRef,
            AlreadyVoted => VoteErrorCode::AlreadyVoted,
        }
    }
}

impl From<CommonError> for VoteError {
    fn from(value: CommonError) -> VoteError {
        VoteError::Common(value)
    }
}

impl From<VoteError> for ExecutionError {
    fn from(value: VoteError) -> ExecutionError {
        ExecutionError::new(value.code() as u8)
    }
}
