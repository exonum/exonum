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

// TODO: Remove attribute when `const fn` becomes stable.
#[cfg_attr(feature = "cargo-clippy", allow(cast_lossless))]
static MAX_RESERVED_VALUE: u16 = ::std::u8::MAX as u16;

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
    /// # use exonum::blockchain::TransactionStatus;
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
    /// #   fn execute(&self, _: &mut Fork) -> TransactionStatus { TransactionStatus::Succeeded }
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
    /// use exonum::blockchain::{Transaction, TransactionStatus};
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
    ///     fn execute(&self, fork: &mut Fork) -> TransactionStatus {
    ///         // Read and/or write into storage.
    ///         // ...
    ///
    ///         // Returns execution status.
    ///         TransactionStatus::Succeeded
    ///     }
    ///
    ///     // Other methods...
    ///     // ...
    /// #   fn verify(&self) -> bool { true }
    /// }
    /// # fn main() {}
    fn execute(&self, fork: &mut Fork) -> TransactionStatus;
}

/// Execution status of the transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TransactionStatus {
    /// Successful transaction execution.
    Succeeded,
    /// Panic occurred during transaction execution. This status should not be used explicitly.
    Panic,
    /// General failure (unspecified reason).
    Failed,
    /// User defined execution status. Can have different meanings for different transactions and
    /// services.
    Custom(u8),
}

impl<'a, T: Transaction> From<T> for Box<Transaction + 'a> {
    fn from(tx: T) -> Self {
        Box::new(tx) as Box<Transaction>
    }
}

impl StorageValue for TransactionStatus {
    fn hash(&self) -> Hash {
        u16::hash(&from_status(*self))
    }

    fn into_bytes(self) -> Vec<u8> {
        u16::into_bytes(from_status(self))
    }

    fn from_bytes(value: Cow<[u8]>) -> Self {
        to_status(u16::from_bytes(value))
    }
}

fn from_status(status: TransactionStatus) -> u16 {
    match status {
        TransactionStatus::Succeeded => 0,
        TransactionStatus::Panic => 1,
        TransactionStatus::Failed => 2,
        TransactionStatus::Custom(value) => u16::from(value) + MAX_RESERVED_VALUE + 1,
    }
}

fn to_status(value: u16) -> TransactionStatus {
    match value {
        0 => TransactionStatus::Succeeded,
        1 => TransactionStatus::Panic,
        2 => TransactionStatus::Failed,
        val => {
            assert!(
                val > MAX_RESERVED_VALUE && (val - MAX_RESERVED_VALUE - 1) <= u8::MAX_VALUE,
                "Invalid ExecutionStatus value: {}",
                val
            );
            TransactionStatus::Custom((val - MAX_RESERVED_VALUE - 1) as u8)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn execution_status_round_trip() {
        let statuses = [
            TransactionStatus::Succeeded,
            TransactionStatus::Panic,
            TransactionStatus::Failed,
            TransactionStatus::Custom(0),
            TransactionStatus::Custom(1),
            TransactionStatus::Custom(100),
            TransactionStatus::Custom(255),
        ];

        for status in &statuses {
            let bytes = status.clone().into_bytes();
            let new_status = TransactionStatus::from_bytes(Cow::Borrowed(&bytes));

            assert_eq!(*status, new_status);
        }
    }
}
