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

//! `Transaction` related types.

use std::borrow::Cow;

use messages::Message;
use storage::{Fork, StorageValue};
use crypto::Hash;
use encoding::serialize::json::ExonumJson;

// There are three `0...255` ranges for each category:
// - `0...255` - special values such as `TransactionValue::Success` or `TransactionError::Panic`.
// - `256...511` - `TransactionValue::Code`.
// - `512...767` - `TransactionError::Code`.
const MIN_SUCCESS: u16 = 256;
const MAX_SUCCESS: u16 = MIN_SUCCESS + 255;
const MIN_ERROR: u16 = MAX_SUCCESS + 1;
const MAX_ERROR: u16 = MIN_ERROR + 255;

// Special reserved values.
const TRANSACTION_SUCCESS: u16 = 0;
const TRANSACTION_PANIC: u16 = 1;
const TRANSACTION_FAILURE: u16 = 2;

/// Result of the `Transaction`'s `execute' method.
pub type TransactionResult = Result<TransactionValue, TransactionError>;

/// A trait that describes transaction processing rules (a group of sequential operations
/// with the Exonum storage) for the given `Message`.
pub trait Transaction: Message + ExonumJson + 'static {
    /// Verifies the transaction, which includes the message signature verification and other
    /// specific internal constraints. verify is intended to check the internal consistency of
    /// a transaction; it has no access to the blockchain state.
    /// If a transaction fails verify, it is considered incorrect and cannot be included into
    /// any correct block proposal. Incorrect transactions are never included into the blockchain.
    ///
    /// *This method should not use external data, that is, it must be a pure function.*
    ///
    /// # Examples
    ///
    /// ```
    /// # #[macro_use] extern crate exonum;
    /// #
    /// use exonum::blockchain::Transaction;
    /// use exonum::crypto::PublicKey;
    /// use exonum::messages::Message;
    /// # use exonum::blockchain::TransactionResult;
    /// # use exonum::storage::Fork;
    ///
    /// message! {
    ///     struct MyTransaction {
    ///         // Transaction definition...
    /// #       const TYPE = 1;
    /// #       const ID = 1;
    /// #       const SIZE = 32;
    ///         field public_key:    &PublicKey    [0 => 32]
    ///     }
    /// }
    ///
    /// impl Transaction for MyTransaction {
    ///     fn verify(&self) -> bool {
    ///         self.verify_signature(self.public_key())
    ///     }
    ///
    ///     // Other methods...
    ///     // ...
    /// #   fn execute(&self, _: &mut Fork) -> TransactionResult {
    /// #       Ok(TransactionValue::Success)
    /// #   }
    /// }
    /// # fn main() {}
    fn verify(&self) -> bool;

    /// Takes the current blockchain state via `fork` and can modify it if certain conditions
    /// are met.
    ///
    /// # Notes
    ///
    /// - When programming `execute`, you should perform state-related checks before any changes
    /// to the state and return early if these checks fail.
    /// - If the execute method of a transaction raises a `panic`, the changes made by the
    /// transactions are discarded, but the transaction itself is still considered committed.
    /// - A transaction execution status (see `ExecutionStatus` for the details) is stored in the
    /// blockchain and can be accessed through api.
    ///
    ///
    /// # Examples
    ///
    /// ```
    /// # #[macro_use] extern crate exonum;
    /// #
    /// use exonum::blockchain::{Transaction, TransactionResult};
    /// use exonum::crypto::PublicKey;
    /// use exonum::storage::Fork;
    ///
    /// message! {
    ///     struct MyTransaction {
    ///         // Transaction definition...
    /// #       const TYPE = 1;
    /// #       const ID = 1;
    /// #       const SIZE = 32;
    /// #       field public_key:    &PublicKey    [0 => 32]
    ///     }
    /// }
    ///
    /// impl Transaction for MyTransaction {
    ///     fn execute(&self, fork: &mut Fork) -> TransactionResult {
    ///         // Read and/or write into storage.
    ///         // ...
    ///
    ///         // Returns execution status.
    ///         Ok(TransactionValue::Success)
    ///     }
    ///
    ///     // Other methods...
    ///     // ...
    /// #   fn verify(&self) -> bool { true }
    /// }
    /// # fn main() {}
    fn execute(&self, fork: &mut Fork) -> TransactionResult;
}

/// Result of successful transaction execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TransactionValue {
    /// Successful transaction execution.
    Success,
    /// User-defined code. Can have different meanings for different transactions and services.
    Code(u8),
}

/// Result of unsuccessful transaction execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TransactionError {
    /// Panic occurred during transaction execution. This status should not be used explicitly.
    Panic,
    /// General failure (unspecified reason).
    UnknownFailure,
    /// User-defined error-code. Can have different meanings for different transactions and
    /// services.
    Code(u8),
}

impl<'a, T: Transaction> From<T> for Box<Transaction + 'a> {
    fn from(tx: T) -> Self {
        Box::new(tx) as Box<Transaction>
    }
}

impl StorageValue for TransactionResult {
    fn hash(&self) -> Hash {
        u16::hash(&from_result(*self))
    }

    fn into_bytes(self) -> Vec<u8> {
        u16::into_bytes(from_result(self))
    }

    fn from_bytes(value: Cow<[u8]>) -> Self {
        to_result(u16::from_bytes(value))
    }
}

fn from_result(result: TransactionResult) -> u16 {
    match result {
        Ok(val) => {
            match val {
                TransactionValue::Success => TRANSACTION_SUCCESS,
                TransactionValue::Code(c) => u16::from(c) + MIN_SUCCESS,
            }
        }
        Err(err) => {
            match err {
                TransactionError::Panic => TRANSACTION_PANIC,
                TransactionError::UnknownFailure => TRANSACTION_FAILURE,
                TransactionError::Code(c) => u16::from(c) + MIN_ERROR,
            }
        }
    }
}

fn to_result(value: u16) -> TransactionResult {
    match value {
        TRANSACTION_SUCCESS => Ok(TransactionValue::Success),
        TRANSACTION_PANIC => Err(TransactionError::Panic),
        TRANSACTION_FAILURE => Err(TransactionError::UnknownFailure),
        val @ MIN_SUCCESS...MAX_SUCCESS => Ok(TransactionValue::Code((val - MIN_SUCCESS) as u8)),
        val @ MIN_ERROR...MAX_ERROR => Err(TransactionError::Code((val - MIN_ERROR) as u8)),
        val => panic!("Invalid TransactionResult value: {}", val),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transaction_result_round_trip() {
        let statuses = [
            Ok(TransactionValue::Success),
            Ok(TransactionValue::Code(0)),
            Ok(TransactionValue::Code(1)),
            Ok(TransactionValue::Code(100)),
            Ok(TransactionValue::Code(254)),
            Ok(TransactionValue::Code(255)),
            Err(TransactionError::Panic),
            Err(TransactionError::UnknownFailure),
            Err(TransactionError::Code(0)),
            Err(TransactionError::Code(1)),
            Err(TransactionError::Code(100)),
            Err(TransactionError::Code(254)),
            Err(TransactionError::Code(255)),
        ];

        for status in &statuses {
            let bytes = status.clone().into_bytes();
            let new_status = TransactionResult::from_bytes(Cow::Borrowed(&bytes));

            assert_eq!(*status, new_status);
        }
    }
}
