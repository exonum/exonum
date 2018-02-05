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
use std::fmt;
use std::u8;

use messages::Message;
use storage::{Fork, StorageValue};
use crypto::{Hash, CryptoHash};
use encoding::serialize::json::ExonumJson;

//  User-defined error codes (`TransactionError::Code(u8)`) have a `0...255` range.
#[cfg_attr(feature = "cargo-clippy", allow(cast_lossless))]
const MAX_ERROR_CODE: u16 = u8::MAX as u16;
// Represent `(Ok())` `TransactionStatus` value.
const TRANSACTION_STATUS_OK: u16 = MAX_ERROR_CODE + 1;
// `Err(TransactionError::Panic)`.
const TRANSACTION_STATUS_PANIC: u16 = TRANSACTION_STATUS_OK + 1;
// `Err(TransactionError::Failure)`.
const TRANSACTION_STATUS_FAILURE: u16 = TRANSACTION_STATUS_PANIC + 1;
// `Err(TransactionError::Description)`.
const TRANSACTION_STATUS_DESCRIPTION: u16 = TRANSACTION_STATUS_FAILURE + 1;

/// Return value of the `Transaction`'s `execute' method. Changes made by the transaction are
/// discarded if `Err` is returned, see `Transaction` documentation for the details.
pub type ExecutionStatus = Result<(), ExecutionError>;
/// Extended version of `ExecutionStatus` (with additional values set exclusively by Exonum
/// framework) that can be obtained through `Schema`'s `transaction_statuses` method.
pub type TransactionStatus = Result<(), TransactionError>;

/// Transaction processing functionality for `Message`s allowing to apply authenticated, atomic,
/// constraint-preserving groups of changes to the blockchain storage.
///
/// See also [the documentation page on transactions][doc:transactions].
///
/// [doc:transactions]: https://exonum.com/doc/architecture/transactions/
pub trait Transaction: Message + ExonumJson + 'static {
    /// Verifies the internal consistency of the transaction. `verify` should usually include
    /// checking the message signature (via [`verify_signature`]) and, possibly,
    /// other internal constraints. `verify` has no access to the blockchain state;
    /// checks involving the blockchains state must be preformed in [`execute`](#tymethod.execute).
    ///
    /// If a transaction fails `verify`,  is considered incorrect and cannot be included into
    /// any correct block proposal. Incorrect transactions are never included into the blockchain.
    ///
    /// *This method should not use external data, that is, it must be a pure function.*
    ///
    /// [`verify_signature`]: ../messages/trait.Message.html#method.verify_signature
    ///
    /// # Examples
    ///
    /// ```
    /// # #[macro_use] extern crate exonum;
    /// #
    /// use exonum::blockchain::Transaction;
    /// use exonum::crypto::PublicKey;
    /// use exonum::messages::Message;
    /// # use exonum::blockchain::ExecutionStatus;
    /// # use exonum::storage::Fork;
    ///
    /// message! {
    ///     struct MyTransaction {
    ///         // Transaction definition...
    /// #       const TYPE = 1;
    /// #       const ID = 1;
    /// #
    ///         public_key: &PublicKey,
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
    /// #   fn execute(&self, _: &mut Fork) -> ExecutionStatus {
    /// #       Ok(())
    /// #   }
    /// }
    /// # fn main() {}
    fn verify(&self) -> bool;

    /// Receives a fork of the current blockchain state and can modify it depending on the contents
    /// of the transaction.
    ///
    /// # Notes
    ///
    /// - Transaction itself is considered committed regardless whether `Ok` or `Err` has been
    ///   returned or even if panic occurred during execution.
    /// - Changes made by the transaction are discarded if `Err` is returned or panic occurred.
    /// - A transaction execution status (see `ExecutionStatus` and `TransactionStatus` for the
    ///   details) is stored in the blockchain and can be accessed through api.
    /// - Blockchain state hash is affected by the transactions execution status.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[macro_use] extern crate exonum;
    /// #
    /// use exonum::blockchain::{Transaction, ExecutionStatus};
    /// use exonum::crypto::PublicKey;
    /// use exonum::storage::Fork;
    ///
    /// message! {
    ///     struct MyTransaction {
    ///         // Transaction definition...
    /// #       const TYPE = 1;
    /// #       const ID = 1;
    /// #
    /// #       public_key: &PublicKey,
    ///     }
    /// }
    ///
    /// impl Transaction for MyTransaction {
    ///     fn execute(&self, fork: &mut Fork) -> ExecutionStatus {
    ///         // Read and/or write into storage.
    ///         // ...
    ///
    ///         // Return execution status.
    ///         Ok(())
    ///     }
    ///
    ///     // Other methods...
    ///     // ...
    /// #   fn verify(&self) -> bool { true }
    /// }
    /// # fn main() {}
    fn execute(&self, fork: &mut Fork) -> ExecutionStatus;
}

/// Result of unsuccessful transaction execution.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ExecutionError {
    /// General failure (unspecified reason).
    Failure,
    /// User-defined error-code. Can have different meanings for different transactions and
    /// services.
    Code(u8),
    /// User-defined string error description.
    Description(String),
}

/// Extended by the framework result of unsuccessful transaction execution.
///
/// # Notes:
///
/// - `Description`'s content excluded from hash calculation (see `StorageValue` implementation for
///   the details).
/// - `TransactionError::Panic` is set by the framework if panic is raised during transaction
///   execution.
/// - `TransactionError` implements `Display` which can be used for obtaining a simple error
///   description.
///
/// # Examples
///
/// ```
/// use exonum::blockchain::TransactionError;
///
/// # let transaction_error = TransactionError::Panic;
/// // Prints user friendly error description.
/// println!("Transaction error: {}", transaction_error);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum TransactionError {
    /// Panic occurred during transaction execution.
    Panic,
    /// General failure (unspecified reason).
    Failure,
    /// User-defined error-code. Can have different meanings for different transactions and
    /// services.
    Code(u8),
    /// User-defined string error description.
    Description(String),
}

/// Converts from `ExecutionStatus` into `TransactionStatus`.
pub fn convert_status(status: ExecutionStatus) -> TransactionStatus {
    match status {
        Ok(()) => Ok(()),
        Err(ExecutionError::Failure) => Err(TransactionError::Failure),
        Err(ExecutionError::Code(c)) => Err(TransactionError::Code(c)),
        Err(ExecutionError::Description(s)) => Err(TransactionError::Description(s)),
    }
}

impl<'a, T: Transaction> From<T> for Box<Transaction + 'a> {
    fn from(tx: T) -> Self {
        Box::new(tx) as Box<Transaction>
    }
}

impl fmt::Display for TransactionError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            TransactionError::Panic => write!(f, "Panic during execution"),
            TransactionError::Failure => write!(f, "Unspecified failure"),
            TransactionError::Code(c) => write!(f, "Error code: {}", c),
            TransactionError::Description(ref s) => write!(f, "{}", s),
        }
    }
}

// String content (`TransactionError::Description`) is intentionally excluded from the hash
// calculation because user can be tempted to use error description from a third-party libraries
// which aren't stable across the versions.
impl CryptoHash for TransactionStatus {
    fn hash(&self) -> Hash {
        u16::hash(&status_as_u16(self))
    }
}

// `TransactionStatus` is stored as `u16` with optional string part needed only for
// `TransactionError::Description`.
impl StorageValue for TransactionStatus {
    fn into_bytes(self) -> Vec<u8> {
        let mut res = u16::into_bytes(status_as_u16(&self));
        match self {
            Err(TransactionError::Description(s)) => {
                res.extend(String::into_bytes(s));
            }
            _ => (),
        }
        res
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        let main_part = u16::from_bytes(Cow::Borrowed(&bytes));
        match main_part {
            value @ 0...MAX_ERROR_CODE => Err(TransactionError::Code(value as u8)),
            TRANSACTION_STATUS_OK => Ok(()),
            TRANSACTION_STATUS_PANIC => Err(TransactionError::Panic),
            TRANSACTION_STATUS_FAILURE => Err(TransactionError::Failure),
            TRANSACTION_STATUS_DESCRIPTION => {
                Err(TransactionError::Description(
                    String::from_bytes(Cow::Borrowed(&bytes[2..])),
                ))
            }
            value => panic!("Invalid TransactionStatus value: {}", value),
        }
    }
}

fn status_as_u16(status: &TransactionStatus) -> u16 {
    match *status {
        Ok(()) => TRANSACTION_STATUS_OK,
        Err(TransactionError::Panic) => TRANSACTION_STATUS_PANIC,
        Err(TransactionError::Failure) => TRANSACTION_STATUS_FAILURE,
        Err(TransactionError::Code(c)) => u16::from(c),
        Err(TransactionError::Description(_)) => TRANSACTION_STATUS_DESCRIPTION,
    }
}

#[cfg(test)]
mod tests {
    use futures::sync::mpsc;

    use std::collections::BTreeMap;
    use std::sync::Mutex;

    use super::*;
    use crypto;
    use blockchain::Blockchain;
    use storage::{Database, MemoryDB, Entry};
    use node::ApiSender;
    use helpers::{ValidatorId, Height};

    lazy_static! {
        static ref EXECUTION_STATUS: Mutex<ExecutionStatus> = Mutex::new(Ok(()));
    }

    #[test]
    fn transaction_status_round_trip() {
        let statuses = [
            Ok(()),
            Err(TransactionError::Panic),
            Err(TransactionError::Failure),
            Err(TransactionError::Code(0)),
            Err(TransactionError::Code(1)),
            Err(TransactionError::Code(100)),
            Err(TransactionError::Code(254)),
            Err(TransactionError::Code(255)),
            Err(TransactionError::Description("".to_owned())),
            Err(TransactionError::Description("e".to_owned())),
            Err(TransactionError::Description("just error".to_owned())),
            Err(TransactionError::Description(
                "(Not) really long error description".to_owned(),
            )),
            Err(TransactionError::Description(
                "_underscored_text_".to_owned(),
            )),
            Err(TransactionError::Description("!@#$%^&*()".to_owned())),
        ];

        for status in &statuses {
            let bytes = status.clone().into_bytes();
            let new_status = TransactionStatus::from_bytes(Cow::Borrowed(&bytes));
            assert_eq!(*status, new_status);
        }
    }

    #[test]
    fn error_discards_transaction_changes() {
        let statuses = [
            Err(ExecutionError::Failure),
            Err(ExecutionError::Code(0)),
            Err(ExecutionError::Code(255)),
            Err(ExecutionError::Description("".to_owned())),
            Err(ExecutionError::Description("Error description".to_owned())),
            Ok(()),
        ];

        let (_, sec_key) = crypto::gen_keypair();
        let (blockchain, mut pool) = create_blockchain();
        let db = Box::new(MemoryDB::new());

        for (index, status) in statuses.iter().enumerate() {
            let index = index as u64;

            *EXECUTION_STATUS.lock().unwrap() = status.clone();

            let transaction = TxResult::new(index, &sec_key);
            pool.insert(
                transaction.hash(),
                Box::new(transaction.clone()) as Box<Transaction>,
            );

            let (_, patch) = blockchain.create_patch(
                ValidatorId::zero(),
                Height(index),
                &[transaction.hash()],
                &pool,
            );

            db.merge(patch).unwrap();

            let mut fork = db.fork();
            let entry = create_entry(&mut fork);
            if status.is_err() {
                assert_eq!(None, entry.get());
            } else {
                assert_eq!(Some(index), entry.get());
            }
        }
    }

    fn create_blockchain() -> (Blockchain, BTreeMap<Hash, Box<Transaction>>) {
        let service_keypair = crypto::gen_keypair();
        let api_channel = mpsc::channel(1);
        (
            Blockchain::new(
                Box::new(MemoryDB::new()),
                Vec::new(),
                service_keypair.0,
                service_keypair.1,
                ApiSender::new(api_channel.0),
            ),
            BTreeMap::new(),
        )
    }

    message! {
        struct TxResult {
            const TYPE = 1;
            const ID = 0;

            index: u64,
        }
    }

    impl Transaction for TxResult {
        fn verify(&self) -> bool {
            true
        }

        fn execute(&self, fork: &mut Fork) -> ExecutionStatus {
            let mut entry = create_entry(fork);
            entry.set(self.index());
            EXECUTION_STATUS.lock().unwrap().clone()
        }
    }

    fn create_entry(fork: &mut Fork) -> Entry<&mut Fork, u64> {
        Entry::new("transaction_status_test", fork)
    }
}
