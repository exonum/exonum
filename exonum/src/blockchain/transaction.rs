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

//! `Transaction` related types.

use std::borrow::Cow;
use std::any::Any;
use std::error::Error;
use std::fmt;
use std::u8;
use std::convert::Into;

use serde::Serialize;
use serde::de::DeserializeOwned;

use messages::{Message, RawTransaction};
use storage::{Fork, StorageValue};
use crypto::{CryptoHash, Hash};
use encoding;
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
    /// transactions! {
    ///     MyTransactions {
    ///         const SERVICE_ID = 1;
    ///
    ///         struct MyTransaction {
    ///             // Transaction definition...
    ///             public_key: &PublicKey,
    ///         }
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
    /// #   fn execute(&self, _: &mut Fork) -> ExecutionResult { Ok(()) }
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
    /// transactions! {
    ///     MyTransactions {
    ///         const SERVICE_ID = 1;
    ///
    ///         struct MyTransaction {
    ///             // Transaction definition...
    ///             public_key: &PublicKey,
    ///         }
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
    /// User-defined error code. Can have different meanings for different transactions and
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
    pub fn with_description<T : Into<String>>(code: u8, description: T) -> Self {
        Self {
            code,
            description: Some(description.into()),
        }
    }
}

/// Type of the transaction error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TransactionErrorType {
    /// Panic occurred during transaction execution.
    Panic,
    /// User-defined error code. Can have different meanings for different transactions and
    /// services.
    Code(u8),
}

/// Result of unsuccessful transaction execution encompassing both service and framework-wide error
/// handling.
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
/// # use exonum::storage::{MemoryDB, Database};
/// # use exonum::crypto::Hash;
/// use exonum::blockchain::Schema;
///
/// # let db = MemoryDB::new();
/// # let snapshot = db.snapshot();
/// # let transaction_hash = Hash::zero();
/// let schema = Schema::new(&snapshot);
///
/// if let Some(result) = schema.transaction_results().get(&transaction_hash) {
///     match result {
///         Ok(()) => println!("Successful transaction execution"),
///         Err(transaction_error) => {
///             // Prints user friendly error description.
///             println!("Transaction error: {}", transaction_error);
///         }
///     }
/// }
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
    pub(crate) fn from_panic(panic: &Box<Any + Send>) -> Self {
        Self::panic(panic_description(panic))
    }

    /// Returns error type of this `TransactionError` instance.
    pub fn error_type(&self) -> TransactionErrorType {
        self.error_type
    }

    /// Returns an optional error description.
    pub fn description(&self) -> Option<&str> {
        self.description.as_ref().map(String::as_ref)
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
        Err(ref e) => match e.error_type {
            TransactionErrorType::Panic => TRANSACTION_STATUS_PANIC,
            TransactionErrorType::Code(c) => u16::from(c),
        },
    }
}

/// `TransactionSet` trait describes a type which is an `enum` of several transactions.
/// The implementation of this trait is generated automatically by the `transactions!`
/// macro.
pub trait TransactionSet
    : Into<Box<Transaction>> + DeserializeOwned + Serialize + Clone {
    /// Parse a transaction from this set from a `RawMessage`.
    fn tx_from_raw(raw: RawTransaction) -> Result<Self, encoding::Error>;
}

/// `transactions!` is used to declare a set of transactions of a particular service.
///
/// The macro generates a type for each transaction and a helper enum which can hold
/// any of the transactions. You must implement `Transaction` trait for each of the
/// transactions yourself.
///
/// See `Service` trait documentation for a full example of usage.
///
/// Each transaction is specified as a Rust struct. For additional reference about
/// data layout see the documentation of the [`encoding` module](./encoding/index.html).
///
/// Additionally, the macro must define identifier of a service, which will be used
/// [in parsing messages][parsing], as `const SERVICE_ID`. Service ID should be unique
/// within the Exonum blockchain.
///
/// For each transaction the macro creates getter methods for all fields with the same names as
/// fields. In addition, two constructors are defined:
///
/// - `new` takes all fields in the order of their declaration in the macro, and a [`SecretKey`]
///   to sign the message as the last argument.
/// - `new_with_signature` takes all fields in the order of their declaration in the macro,
///   and a message [`Signature`].
///
/// Each transaction also implements [`Message`], [`ServiceMessage`], [`SegmentField`],
/// [`ExonumJson`] and [`StorageValue`] traits for the declared datatype.
///
///
/// **NB.** `transactions!` uses other macros in the `exonum` crate internally.
/// Be sure to add them to the global scope.
///
/// [`Transaction`]: ./blockchain/trait.Transaction.html
/// [parsing]: ./blockchain/trait.Service.html#tymethod.tx_from_raw
/// [`SecretKey`]: ./crypto/struct.SecretKey.html
/// [`Signature`]: ./crypto/struct.Signature.html
/// [`SegmentField`]: ./encoding/trait.SegmentField.html
/// [`ExonumJson`]: ./encoding/serialize/json/trait.ExonumJson.html
/// [`StorageValue`]: ./storage/trait.StorageValue.html
/// [`Message`]: ./messages/trait.Message.html
/// [`ServiceMessage`]: ./messages/trait.ServiceMessage.html
/// # Examples
///
/// ```
/// #[macro_use] extern crate exonum;
/// use exonum::crypto::PublicKey;
/// # use exonum::storage::Fork;
/// # use exonum::blockchain::{Transaction, ExecutionResult};
///
/// transactions! {
///     WalletTransactions {
///         const SERVICE_ID = 1;
///
///         struct Create {
///             key: &PublicKey
///         }
///
///         struct Transfer {
///             from: &PublicKey,
///             to: &PublicKey,
///             amount: u64,
///         }
///     }
/// }
/// # impl Transaction for Create {
/// #   fn verify(&self) -> bool { true }
/// #   fn execute(&self, fork: &mut Fork) -> ExecutionResult { Ok(()) }
/// # }
/// #
/// # impl Transaction for Transfer {
/// #   fn verify(&self) -> bool { true }
/// #   fn execute(&self, fork: &mut Fork) -> ExecutionResult { Ok(()) }
/// # }
/// #
/// # fn main() { }
/// ```
#[macro_export]
macro_rules! transactions {
    // Variant with the private enum.
    {
        $(#[$tx_set_attr:meta])*
        $transaction_set:ident {
            const SERVICE_ID = $service_id:expr;

            $(
                $(#[$tx_attr:meta])*
                struct $name:ident {
                    $($def:tt)*
                }
            )*
        }
    } => {
        messages! {
            const SERVICE_ID = $service_id;
            $(
                $(#[$tx_attr])*
                struct $name {
                    $($def)*
                }
            )*
        }

        #[derive(Clone, Debug)]
        $(#[$tx_set_attr])*
        enum $transaction_set {
            $(
                #[allow(missing_docs)]
                $name($name),
            )*
        }

        transactions!(@implement $transaction_set, $($name)*);
    };
    // Variant with the public enum without restrictions.
    {
        $(#[$tx_set_attr:meta])*
        pub $transaction_set:ident {
            const SERVICE_ID = $service_id:expr;

            $(
                $(#[$tx_attr:meta])*
                struct $name:ident {
                    $($def:tt)*
                }
            )*
        }
    } => {
        messages! {
            const SERVICE_ID = $service_id;
            $(
                $(#[$tx_attr])*
                struct $name {
                    $($def)*
                }
            )*
        }

        #[derive(Clone, Debug)]
        $(#[$tx_set_attr])*
        pub enum $transaction_set {
            $(
                #[allow(missing_docs)]
                $name($name),
            )*
        }

        transactions!(@implement $transaction_set, $($name)*);
    };
    // Variant with the public enum with visibility restrictions.
    {
        $(#[$tx_set_attr:meta])*
        pub($($vis:tt)+) $transaction_set:ident {
            const SERVICE_ID = $service_id:expr;

            $(
                $(#[$tx_attr:meta])*
                struct $name:ident {
                    $($def:tt)*
                }
            )*
        }
    } => {
        messages! {
            const SERVICE_ID = $service_id;
            $(
                $(#[$tx_attr])*
                struct $name {
                    $($def)*
                }
            )*
        }

        #[derive(Clone, Debug)]
        $(#[$tx_set_attr])*
        pub($($vis)+) enum $transaction_set {
            $(
                #[allow(missing_docs)]
                $name($name),
            )*
        }

        transactions!(@implement $transaction_set, $($name)*);
    };
    // Implementation details
    (@implement $transaction_set:ident, $($name:ident)*) => {

        impl $crate::blockchain::TransactionSet for $transaction_set {
            fn tx_from_raw(
                raw: $crate::messages::RawTransaction
            ) -> ::std::result::Result<Self, $crate::encoding::Error> {
                let message_type = raw.message_type();
                match message_type {
                    $(
                    <$name as $crate::messages::ServiceMessage>::MESSAGE_ID => {
                        let tx = $crate::messages::Message::from_raw(raw)?;
                        Ok($transaction_set::$name(tx))
                    }
                    )*
                    _ => return Err($crate::encoding::Error::IncorrectMessageType { message_type })
                }
            }
        }

        impl Into<Box<$crate::blockchain::Transaction>> for $transaction_set {
            fn into(self) -> Box<$crate::blockchain::Transaction> {
                match self {$(
                    $transaction_set::$name(tx) => Box::new(tx),
                )*}
            }
        }

        impl<'de> $crate::encoding::serialize::reexport::Deserialize<'de> for $transaction_set {
            fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
            where
                D: $crate::encoding::serialize::reexport::Deserializer<'de>,
            {
                use $crate::encoding::serialize::json::reexport::{Value, from_value};
                use $crate::encoding::serialize::reexport::{DeError, Deserialize};

                let value = <Value as Deserialize>::deserialize(deserializer)?;
                let message_id: Value = value.get("message_id")
                    .ok_or(D::Error::custom("Can't get message_id from json"))?
                    .clone();
                let message_id: u16 = from_value(message_id)
                    .map_err(|e| D::Error::custom(
                        format!("Can't deserialize message_id: {}", e)
                    ))?;

                match message_id {
                    $(
                    <$name as $crate::messages::ServiceMessage>::MESSAGE_ID =>
                        <$name as $crate::encoding::serialize::json::ExonumJsonDeserialize>
                            ::deserialize(&value)
                            .map_err(|e| D::Error::custom(
                                format!("Can't deserialize a value: {}", e.description())
                            ))
                            .map($transaction_set::$name),
                    )*
                    _ => Err(D::Error::custom(format!("invalid message_id: {}", message_id))),
                }
            }
        }

        impl $crate::encoding::serialize::reexport::Serialize for $transaction_set {
            fn serialize<S>(&self, serializer: S) -> ::std::result::Result<S::Ok, S::Error>
            where
                S: $crate::encoding::serialize::reexport::Serializer,
            {
                use $crate::encoding::serialize::reexport::Serialize;

                match self {$(
                    &$transaction_set::$name(ref tx) => Serialize::serialize(tx, serializer),
                )*}
            }
        }
    };
}

/// Tries to get a meaningful description from the given panic.
fn panic_description(any: &Box<Any + Send>) -> Option<String> {
    if let Some(s) = any.downcast_ref::<&str>() {
        Some(s.to_string())
    } else if let Some(s) = any.downcast_ref::<String>() {
        Some(s.clone())
    } else if let Some(error) = any.downcast_ref::<Box<Error + Send>>() {
        Some(error.description().to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use futures::sync::mpsc;

    use std::sync::Mutex;
    use std::panic;

    use super::*;
    use crypto;
    use encoding;
    use blockchain::{Blockchain, Schema, Service};
    use storage::{Database, Entry, MemoryDB, Snapshot};
    use node::ApiSender;
    use helpers::{Height, ValidatorId};

    const TX_RESULT_SERVICE_ID: u16 = 255;

    lazy_static! {
        static ref EXECUTION_STATUS: Mutex<ExecutionResult> = Mutex::new(Ok(()));
    }

    #[test]
    fn execution_error_new() {
        let codes = [0, 1, 100, 255];

        for &code in &codes {
            let error = ExecutionError::new(code);
            assert_eq!(code, error.code);
            assert_eq!(None, error.description);
        }
    }

    #[test]
    fn execution_error_with_description() {
        let values = [(0, ""), (1, "test"), (100, "error"), (255, "hello")];

        for value in &values {
            let error = ExecutionError::with_description(value.0, value.1);
            assert_eq!(value.0, error.code);
            assert_eq!(value.1, error.description.unwrap());
        }
    }


    #[test]
    fn transaction_error_new() {
        let values = [
            (TransactionErrorType::Panic, None),
            (TransactionErrorType::Panic, Some("panic")),
            (TransactionErrorType::Code(0), None),
            (TransactionErrorType::Code(1), Some("")),
            (TransactionErrorType::Code(100), None),
            (TransactionErrorType::Code(255), Some("error description")),
        ];

        for value in &values {
            let error = TransactionError::new(value.0, value.1.map(str::to_owned));
            assert_eq!(value.0, error.error_type());
            assert_eq!(value.1.as_ref().map(|d| d.as_ref()), error.description());
        }
    }

    #[test]
    fn errors_conversion() {
        let execution_errors = [
            ExecutionError::new(0),
            ExecutionError::new(255),
            ExecutionError::with_description(1, ""),
            ExecutionError::with_description(1, "Terrible failure"),
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
            Err(TransactionError::panic(Some(
                "Panic error description".to_owned(),
            ))),
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
                "Strange error",
            )),
            Err(ExecutionError::new(255)),
            Err(ExecutionError::with_description(
                255,
                "Error description...",
            )),
            Ok(()),
        ];

        let (_, sec_key) = crypto::gen_keypair();
        let mut blockchain = create_blockchain();
        let db = Box::new(MemoryDB::new());

        for (index, status) in statuses.iter().enumerate() {
            let index = index as u64;

            *EXECUTION_STATUS.lock().unwrap() = status.clone();

            let transaction = TxResult::new(index, &sec_key);
            let hash = transaction.hash();
            {
                let mut fork = blockchain.fork();
                {
                    let mut schema = Schema::new(&mut fork);
                    schema.add_transaction_into_pool(transaction.raw().clone());
                }
                blockchain.merge(fork.into_patch()).unwrap();
            }

            let (_, patch) = blockchain.create_patch(ValidatorId::zero(), Height(index), &[hash]);

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

    #[test]
    fn str_panic() {
        let static_str = "Static string (&str)";
        let panic = make_panic(static_str);
        assert_eq!(Some(static_str.to_string()), panic_description(&panic));
    }

    #[test]
    fn string_panic() {
        let string = "Owned string (String)".to_owned();
        let error = make_panic(string.clone());
        assert_eq!(Some(string), panic_description(&error));
    }

    #[test]
    fn box_error_panic() {
        let error: Box<Error + Send> = Box::new("e".parse::<i32>().unwrap_err());
        let description = error.description().to_owned();
        let error = make_panic(error);
        assert_eq!(Some(description), panic_description(&error));
    }

    #[test]
    fn unknown_panic() {
        let error = make_panic(1);
        assert_eq!(None, panic_description(&error));
    }

    fn make_panic<T: Send + 'static>(val: T) -> Box<Any + Send> {
        panic::catch_unwind(panic::AssertUnwindSafe(|| panic!(val))).unwrap_err()
    }

    fn create_blockchain() -> Blockchain {
        let service_keypair = crypto::gen_keypair();
        let api_channel = mpsc::channel(1);
        Blockchain::new(
            MemoryDB::new(),
            vec![Box::new(TxResultService) as Box<Service>],
            service_keypair.0,
            service_keypair.1,
            ApiSender::new(api_channel.0),
        )
    }

    struct TxResultService;

    impl Service for TxResultService {
        fn service_id(&self) -> u16 {
            TX_RESULT_SERVICE_ID
        }

        fn service_name(&self) -> &'static str {
            "test service"
        }

        fn state_hash(&self, _: &Snapshot) -> Vec<Hash> {
            vec![]
        }

        fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, encoding::Error> {
            Ok(Box::new(TxResult::from_raw(raw)?))
        }
    }

    transactions! {
        TestTxs {
            const SERVICE_ID = TX_RESULT_SERVICE_ID;

            struct TxResult {
                index: u64,
            }
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
