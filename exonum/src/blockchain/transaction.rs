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

//! `Transaction` related types.

use exonum_merkledb::{BinaryValue, ObjectHash};
use protobuf::Message;

use std::{any::Any, borrow::Cow, convert::Into, error::Error, fmt, u8};

use crate::{
    crypto::Hash,
    proto::{self, ProtobufConvert},
};

//  User-defined error codes (`TransactionErrorType::Code(u8)`) have a `0...255` range.
const MAX_ERROR_CODE: u16 = u8::max_value() as u16;
// Represent `(Ok())` `TransactionResult` value.
const TRANSACTION_STATUS_OK: u16 = MAX_ERROR_CODE + 1;
// `Err(TransactionErrorType::Panic)`.
const TRANSACTION_STATUS_PANIC: u16 = TRANSACTION_STATUS_OK + 1;

/// Returns a result of the `Transaction` `execute` method. This result may be
/// either an empty unit type, in case of success, or an `ExecutionError`, if execution has
/// failed. Errors consist of an error code and an optional description.
pub type ExecutionResult = Result<(), ExecutionError>;
/// Extended version of `ExecutionResult` (with additional values set exclusively by Exonum
/// framework) that can be obtained through `Schema::transaction_results` method.
#[derive(Clone, Debug, PartialEq)]
pub struct TransactionResult(pub Result<(), TransactionError>);

/// Result of unsuccessful transaction execution.
///
/// An execution error consists
/// of an error code and optional description. The error code affects the blockchain
/// state hash, while the description does not. Therefore,
/// descriptions are mostly used for developer purposes, not for interaction of
/// the system with users.
///
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ExecutionError {
    /// User-defined error code. Error codes can have different meanings for different
    /// transactions and services.
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
    pub fn with_description<T: Into<String>>(code: u8, description: T) -> Self {
        Self {
            code,
            description: Some(description.into()),
        }
    }
}

/// Type of transaction error.
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
/// This error indicates whether a panic or a user error has occurred.
///
/// # Notes:
///
/// - Content of the `description` field is excluded from the hash calculation (see `StorageValue`
///   implementation for the details).
/// - `TransactionErrorType::Panic` is set by the framework if panic is raised during transaction
///   execution.
/// - `TransactionError` implements `Display` which can be used for obtaining a simple error
///   description.
///
/// # Examples
///
/// The example below creates a schema; retrieves the table
/// with transaction results from this schema; using a hash takes the result
/// of a certain transaction and returns a message that depends on whether the
/// transaction is successful or not.
///
/// ```
/// # use exonum_merkledb::{TemporaryDB, Database};
/// # use exonum::crypto::Hash;
/// use exonum::blockchain::Schema;
///
/// # let db = TemporaryDB::new();
/// # let snapshot = db.snapshot();
/// # let transaction_hash = Hash::zero();
/// let schema = Schema::new(&snapshot);
///
/// if let Some(result) = schema.transaction_results().get(&transaction_hash) {
///     match result.0 {
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
    pub(crate) fn code(code: u8, description: Option<String>) -> Self {
        Self::new(TransactionErrorType::Code(code), description)
    }

    /// Creates a new `TransactionError` representing panic with the given description.
    pub(crate) fn panic(description: Option<String>) -> Self {
        Self::new(TransactionErrorType::Panic, description)
    }

    /// Creates a new `TransactionError` instance from `std::thread::Result`'s `Err`.
    pub(crate) fn from_panic(panic: &Box<dyn Any + Send>) -> Self {
        Self::panic(panic_description(panic))
    }

    /// Returns an error type of this `TransactionError` instance. This can be
    /// either a panic or a user-defined error code.
    pub fn error_type(&self) -> TransactionErrorType {
        self.error_type
    }

    /// Returns an optional error description.
    pub fn description(&self) -> Option<&str> {
        self.description.as_ref().map(String::as_ref)
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
impl ObjectHash for TransactionResult {
    fn object_hash(&self) -> Hash {
        u16::object_hash(&status_as_u16(self))
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

impl ProtobufConvert for TransactionResult {
    type ProtoStruct = proto::TransactionResult;

    fn to_pb(&self) -> Self::ProtoStruct {
        let mut proto = <Self as ProtobufConvert>::ProtoStruct::new();
        proto.set_status(status_as_u16(self).to_pb());
        if let Some(description) = self.0.as_ref().err().and_then(|e| e.description.clone()) {
            proto.set_description(description);
        }
        proto
    }

    fn from_pb(mut pb: Self::ProtoStruct) -> Result<Self, failure::Error> {
        let status_code: u16 = ProtobufConvert::from_pb(pb.get_status())?;
        let description = if pb.get_description() != "" {
            Some(pb.take_description())
        } else {
            None
        };

        Ok(TransactionResult(match status_code {
            value @ 0..=MAX_ERROR_CODE => Err(TransactionError::code(value as u8, description)),
            TRANSACTION_STATUS_OK => Ok(()),
            TRANSACTION_STATUS_PANIC => Err(TransactionError::panic(description)),
            value => bail!("Invalid TransactionResult value: {}", value),
        }))
    }
}

impl BinaryValue for TransactionResult {
    fn to_bytes(&self) -> Vec<u8> {
        self.to_pb()
            .write_to_bytes()
            .expect("Failed to serialize TransactionResult to protobuf.")
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Result<Self, failure::Error> {
        let mut block = <Self as ProtobufConvert>::ProtoStruct::new();
        block
            .merge_from_bytes(bytes.as_ref())
            .expect("Failed to parse TransactionResult from protobuf.");
        ProtobufConvert::from_pb(block)
    }
}

fn status_as_u16(status: &TransactionResult) -> u16 {
    match (*status).0 {
        Ok(()) => TRANSACTION_STATUS_OK,
        Err(ref e) => match e.error_type {
            TransactionErrorType::Panic => TRANSACTION_STATUS_PANIC,
            TransactionErrorType::Code(c) => u16::from(c),
        },
    }
}

/// Tries to get a meaningful description from the given panic.
fn panic_description(any: &Box<dyn Any + Send>) -> Option<String> {
    if let Some(s) = any.downcast_ref::<&str>() {
        Some(s.to_string())
    } else if let Some(s) = any.downcast_ref::<String>() {
        Some(s.clone())
    } else if let Some(error) = any.downcast_ref::<Box<dyn Error + Send>>() {
        Some(error.description().to_string())
    } else {
        None
    }
}

// TODO move error tests to runtime module [ECR-3236]
#[cfg(test)]
mod tests {
    use exonum_merkledb::{Database, Entry, Fork, TemporaryDB};
    use futures::sync::mpsc;
    use semver::Version;

    use std::{panic, sync::Mutex};

    use crate::{
        blockchain::{Blockchain, ExecutionResult, InstanceCollection, Schema},
        crypto,
        helpers::{generate_testnet_config, Height, ValidatorId},
        impl_service_dispatcher,
        messages::ServiceInstanceId,
        node::ApiSender,
        proto::schema::tests::TestServiceTx,
        runtime::rust::{RustArtifactId, Service, ServiceFactory, Transaction, TransactionContext},
    };

    use super::*;

    const TX_CHECK_RESULT_SERVICE_ID: ServiceInstanceId = 255;

    lazy_static! {
        static ref EXECUTION_STATUS: Mutex<ExecutionResult> = Mutex::new(Ok(()));
    }

    #[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert)]
    #[exonum(pb = "TestServiceTx", crate = "crate")]
    struct TxResult {
        value: u64,
    }

    #[derive(Debug)]
    struct TxResultCheckService;

    #[service_interface(exonum(crate = "crate"))]
    trait TxResultCheckInterface {
        fn tx_result(&self, context: TransactionContext, arg: TxResult) -> ExecutionResult;
    }

    impl_service_dispatcher!(TxResultCheckService, TxResultCheckInterface);

    impl TxResultCheckInterface for TxResultCheckService {
        fn tx_result(&self, context: TransactionContext, arg: TxResult) -> ExecutionResult {
            let mut entry = create_entry(context.fork());
            entry.set(arg.value);
            EXECUTION_STATUS.lock().unwrap().clone()
        }
    }

    impl Service for TxResultCheckService {}

    impl ServiceFactory for TxResultCheckService {
        fn artifact_id(&self) -> RustArtifactId {
            RustArtifactId {
                name: "tx_result_check_service".into(),
                version: Version::new(1, 0, 0),
            }
        }

        fn create_instance(&self) -> Box<dyn Service> {
            Box::new(Self)
        }
    }

    fn create_blockchain() -> Blockchain {
        let config = generate_testnet_config(1, 0)[0].clone();
        let service_keypair = config.service_keypair();
        let api_channel = mpsc::unbounded();
        let internal_sender = mpsc::channel(1).0;

        Blockchain::new(
            TemporaryDB::new(),
            vec![InstanceCollection::new(TxResultCheckService).with_instance(
                TX_CHECK_RESULT_SERVICE_ID,
                "tx-check-service",
                (),
            )],
            config.genesis,
            service_keypair,
            ApiSender::new(api_channel.0),
            internal_sender,
        )
    }

    fn create_entry(fork: &Fork) -> Entry<&Fork, u64> {
        Entry::new("transaction_status_test", fork)
    }

    fn make_panic<T: Send + 'static>(val: T) -> Box<dyn Any + Send> {
        panic::catch_unwind(panic::AssertUnwindSafe(|| panic!(val))).unwrap_err()
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
        let values = vec![
            (TransactionErrorType::Panic, None),
            (TransactionErrorType::Panic, Some("panic")),
            (TransactionErrorType::Code(0), None),
            (TransactionErrorType::Code(1), Some("")),
            (TransactionErrorType::Code(100), None),
            (TransactionErrorType::Code(255), Some("error description")),
        ];

        for (err_type, description) in values {
            let error = TransactionError::new(err_type, description.map(str::to_owned));
            assert_eq!(err_type, error.error_type());
            assert_eq!(description, error.description());
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
            Err(TransactionError::panic(Some(
                "Panic error description".to_owned(),
            ))),
            Err(TransactionError::code(0, None)),
            Err(TransactionError::code(
                0,
                Some("Some error description".to_owned()),
            )),
            Err(TransactionError::code(1, None)),
            Err(TransactionError::code(100, None)),
            Err(TransactionError::code(100, Some("just error".to_owned()))),
            Err(TransactionError::code(254, None)),
            Err(TransactionError::code(254, Some("e".to_owned()))),
            Err(TransactionError::code(255, None)),
            Err(TransactionError::code(
                255,
                Some("(Not) really long error description".to_owned()),
            )),
        ]
        .iter()
        .map(|res| TransactionResult(res.to_owned()))
        .collect::<Vec<_>>();

        for result in &results {
            let bytes = result.clone().into_bytes();
            let new_result = TransactionResult::from_bytes(Cow::Borrowed(&bytes))
                .expect("Error while deserializing value");
            assert_eq!(*result, new_result);
        }
    }

    #[test]
    fn error_discards_transaction_changes() {
        let statuses = [
            Err(ExecutionError::new(0)),
            Err(ExecutionError::with_description(0, "Strange error")),
            Err(ExecutionError::new(255)),
            Err(ExecutionError::with_description(
                255,
                "Error description...",
            )),
            Ok(()),
        ];

        let (pk, sec_key) = crypto::gen_keypair();
        let mut blockchain = create_blockchain();
        let db = TemporaryDB::new();

        for (index, status) in statuses.iter().enumerate() {
            let index = index as u64;

            *EXECUTION_STATUS.lock().unwrap() = status.clone();

            let transaction =
                TxResult { value: index }.sign(TX_CHECK_RESULT_SERVICE_ID, pk, &sec_key);
            let hash = transaction.object_hash();
            {
                let fork = blockchain.fork();
                {
                    let mut schema = Schema::new(&fork);
                    schema.add_transaction_into_pool(transaction.clone());
                }
                blockchain.merge(fork.into_patch()).unwrap();
            }

            let (_, patch) = blockchain.create_patch(ValidatorId::zero(), Height(index), &[hash]);

            db.merge(patch).unwrap();

            let fork = db.fork();
            let entry = create_entry(&fork);
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
        let error: Box<dyn Error + Send> = Box::new("e".parse::<i32>().unwrap_err());
        let description = error.description().to_owned();
        let error = make_panic(error);
        assert_eq!(Some(description), panic_description(&error));
    }

    #[test]
    fn unknown_panic() {
        let error = make_panic(1);
        assert_eq!(None, panic_description(&error));
    }
}
