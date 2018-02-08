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
use std::any::Any;
use std::fmt;
use std::u8;

use messages::Message;
use storage::{Fork, StorageValue};
use crypto::{Hash, CryptoHash};
use encoding::serialize::json::ExonumJson;

//  User-defined error codes (`TransactionErrorType::Code(u8)`) have a `0...255` range.
#[cfg_attr(feature = "cargo-clippy", allow(cast_lossless))]
const MAX_ERROR_CODE: u16 = u8::MAX as u16;
// Represent `(Ok())` `TransactionResult` value.
const TRANSACTION_STATUS_OK: u16 = MAX_ERROR_CODE + 1;
// `Err(TransactionErrorType::Panic)`.
const TRANSACTION_STATUS_PANIC: u16 = TRANSACTION_STATUS_OK + 1;

/// Return value of the `Transaction`'s `execute' method. Changes made by the transaction are
/// discarded if `Err` is returned, see `Transaction` documentation for the details.
pub type ExecutionResult = Result<(), ExecutionError>;
/// Extended version of `ExecutionResult` (with additional values set exclusively by Exonum
/// framework) that can be obtained through `Schema`'s `transaction_statuses` method.
pub type TransactionResult = Result<(), TransactionError>;

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
    /// # use exonum::blockchain::ExecutionResult;
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
    /// #   fn execute(&self, _: &mut Fork) -> ExecutionResult {
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
    /// - A transaction execution status (see `ExecutionResult` and `TransactionResult` for the
    ///   details) is stored in the blockchain and can be accessed through api.
    /// - Blockchain state hash is affected by the transactions execution status.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[macro_use] extern crate exonum;
    /// #
    /// use exonum::blockchain::{Transaction, ExecutionResult};
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
    ///     fn execute(&self, fork: &mut Fork) -> ExecutionResult {
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
    fn execute(&self, fork: &mut Fork) -> ExecutionResult;
}

/// Result of unsuccessful transaction execution.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ExecutionError {
    /// User-defined error-code. Can have different meanings for different transactions and
    /// services.
    code: u8,
    /// Optional error description.
    description: Option<String>,
}

impl ExecutionError {
    /// Constructs a new `ExecutionError` instance with the given error code.
    pub fn new(code: u8) -> Self {
        Self {
            code,
            description: None,
        }
    }

    /// Constructs a new `ExecutionError` instance with the given error code and description.
    pub fn with_description(code: u8, description: String) -> Self {
        Self {
            code,
            description: Some(description),
        }
    }
}

/// Type of the transaction error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TransactionErrorType {
    /// Panic occurred during transaction execution.
    Panic,
    /// User-defined error-code. Can have different meanings for different transactions and
    /// services.
    Code(u8),
}

/// Extended by the framework result of unsuccessful transaction execution.
///
/// # Notes:
///
/// - Content of `description`' field is excluded from hash calculation (see `StorageValue`
///   implementation for the details).
/// - `TransactionErrorType::Panic` is set by the framework if panic is raised during transaction
///   execution.
/// - `TransactionError` implements `Display` which can be used for obtaining a simple error
///   description.
///
/// # Examples
///
/// ```
/// # use exonum::blockchain::{ExecutionError, TransactionError};
///
/// # let transaction_error: TransactionError = ExecutionError::new(1).into();
/// // Prints user friendly error description.
/// println!("Transaction error: {}", transaction_error);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TransactionError {
    /// Error type, see `TransactionErrorType` for the details.
    error_type: TransactionErrorType,
    /// Optional error description.
    description: Option<String>,
}

impl TransactionError {
    /// Creates a new `TransactionError` instance with the specified error type and description.
    fn new(error_type: TransactionErrorType, description: Option<String>) -> Self {
        Self {
            error_type,
            description,
        }
    }

    /// Creates a new `TransactionError` instance with the specified error code and description.
    fn code(code: u8, description: Option<String>) -> Self {
        Self::new(TransactionErrorType::Code(code), description)
    }

    /// Creates a new `TransactionError` representing panic with the given description.
    fn panic(description: Option<String>) -> Self {
        Self::new(TransactionErrorType::Panic, description)
    }

    /// Creates a new `TransactionError` instance from `std::thread::Result`'s `Err`.
    pub(crate) fn from_panic(_panic: &Box<Any + Send>) -> Self {
        // TODO: Try to get description from panic.
        Self::panic(None)
    }

    /// Returns error type of this `TransactionError` instance.
    pub fn error_type(&self) -> TransactionErrorType {
        self.error_type
    }

    /// Returns an optional error description.
    pub fn description(&self) -> &Option<String> {
        &self.description
    }
}

impl<'a, T: Transaction> From<T> for Box<Transaction + 'a> {
    fn from(tx: T) -> Self {
        Box::new(tx) as Box<Transaction>
    }
}

impl fmt::Display for TransactionError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.error_type {
            TransactionErrorType::Panic => write!(f, "Panic during execution")?,
            TransactionErrorType::Code(c) => write!(f, "Error code: {}", c)?,
        }

        if let Some(ref description) = self.description {
            write!(f, " description: {}", description)?;
        }

        Ok(())
    }
}

// String content (`TransactionError::Description`) is intentionally excluded from the hash
// calculation because user can be tempted to use error description from a third-party libraries
// which aren't stable across the versions.
impl CryptoHash for TransactionResult {
    fn hash(&self) -> Hash {
        u16::hash(&status_as_u16(self))
    }
}

impl From<ExecutionError> for TransactionError {
    fn from(error: ExecutionError) -> Self {
        Self {
            error_type: TransactionErrorType::Code(error.code),
            description: error.description,
        }
    }
}

// `TransactionResult` is stored as `u16` plus `bool` (`true` means that optional part is present)
// with optional string part needed only for string error description.
impl StorageValue for TransactionResult {
    fn into_bytes(self) -> Vec<u8> {
        let mut res = u16::into_bytes(status_as_u16(&self));
        if let Some(description) = self.err().and_then(|e| e.description) {
            res.extend(bool::into_bytes(true));
            res.extend(String::into_bytes(description));
        } else {
            res.extend(bool::into_bytes(false));
        }
        res
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        let main_part = u16::from_bytes(Cow::Borrowed(&bytes));
        let description = if bool::from_bytes(Cow::Borrowed(&bytes[2..3])) {
            Some(String::from_bytes(Cow::Borrowed(&bytes[3..])))
        } else {
            None
        };

        match main_part {
            value @ 0...MAX_ERROR_CODE => Err(TransactionError::code(value as u8, description)),
            TRANSACTION_STATUS_OK => Ok(()),
            TRANSACTION_STATUS_PANIC => Err(TransactionError::panic(description)),
            value => panic!("Invalid TransactionResult value: {}", value),
        }
    }
}

fn status_as_u16(status: &TransactionResult) -> u16 {
    match *status {
        Ok(()) => TRANSACTION_STATUS_OK,
        Err(ref e) => {
            match e.error_type {
                TransactionErrorType::Panic => TRANSACTION_STATUS_PANIC,
                TransactionErrorType::Code(c) => u16::from(c),
            }
        }
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
        static ref EXECUTION_STATUS: Mutex<ExecutionResult> = Mutex::new(Ok(()));
    }

    #[test]
    fn errors_conversion() {
        let execution_errors = [
            ExecutionError::new(0),
            ExecutionError::new(255),
            ExecutionError::with_description(1, "".to_owned()),
            ExecutionError::with_description(1, "Terrible failure".to_owned()),
        ];

        for execution_error in &execution_errors {
            let transaction_error: TransactionError = execution_error.clone().into();
            assert_eq!(execution_error.description, transaction_error.description);

            let code = match transaction_error.error_type {
                TransactionErrorType::Code(c) => c,
                _ => panic!("Unexpected transaction error type"),
            };
            assert_eq!(execution_error.code, code);
        }
    }

    #[test]
    fn transaction_results_round_trip() {
        let results = [
            Ok(()),
            Err(TransactionError::panic(None)),
            Err(TransactionError::panic(Some("".to_owned()))),
            Err(TransactionError::panic(
                Some("Panic error description".to_owned()),
            )),
            Err(TransactionError::code(0, None)),
            Err(TransactionError::code(
                0,
                Some("Some error description".to_owned()),
            )),
            Err(TransactionError::code(1, None)),
            Err(TransactionError::code(1, Some("".to_owned()))),
            Err(TransactionError::code(100, None)),
            Err(TransactionError::code(100, Some("just error".to_owned()))),
            Err(TransactionError::code(254, None)),
            Err(TransactionError::code(254, Some("e".to_owned()))),
            Err(TransactionError::code(255, None)),
            Err(TransactionError::code(
                255,
                Some("(Not) really long error description".to_owned()),
            )),
        ];

        for result in &results {
            let bytes = result.clone().into_bytes();
            let new_result = TransactionResult::from_bytes(Cow::Borrowed(&bytes));
            assert_eq!(*result, new_result);
        }
    }

    #[test]
    fn error_discards_transaction_changes() {
        let statuses = [
            Err(ExecutionError::new(0)),
            Err(ExecutionError::with_description(
                0,
                "Strange error".to_owned(),
            )),
            Err(ExecutionError::new(255)),
            Err(ExecutionError::with_description(
                255,
                "Error description...".to_owned(),
            )),
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

        fn execute(&self, fork: &mut Fork) -> ExecutionResult {
            let mut entry = create_entry(fork);
            entry.set(self.index());
            EXECUTION_STATUS.lock().unwrap().clone()
        }
    }

    fn create_entry(fork: &mut Fork) -> Entry<&mut Fork, u64> {
        Entry::new("transaction_status_test", fork)
    }
}
