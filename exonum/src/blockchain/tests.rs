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

#![allow(dead_code, unsafe_code)]

use exonum_merkledb::{
    Database, DbOptions, Error as StorageError, Fork, ListIndex, ObjectHash, RocksDB, Snapshot,
    TemporaryDB,
};
use futures::sync::mpsc;
use rand::{distributions::Alphanumeric, thread_rng, Rng};

use crate::{
    blockchain::{Blockchain, ExecutionResult, Schema},
    crypto::{gen_keypair, Hash},
    helpers::{Height, ValidatorId},
    impl_service_dispatcher,
    messages::{AnyTx, BinaryValue, Message, ServiceInstanceId, ServiceTransaction},
    node::ApiSender,
    proto::schema::tests::*,
    runtime::{
        dispatcher::DispatcherBuilder,
        rust::{RustArtifactSpec, Service, ServiceFactory, TransactionContext},
    },
};

const IDX_NAME: &str = "idx_name";
const TEST_SERVICE_ID: ServiceInstanceId = 255;

#[service_interface(exonum(crate = "crate"))]
trait TestService {
    fn tx(&self, context: TransactionContext, arg: Tx) -> ExecutionResult;
}

#[derive(Debug)]
struct TestServiceImpl;

impl TestService for TestServiceImpl {
    fn tx(&self, context: TransactionContext, arg: Tx) -> ExecutionResult {
        if arg.value == 42 {
            panic!(StorageError::new("42"))
        }
        let mut index = ListIndex::new(IDX_NAME, context.fork());
        index.push(arg.value);
        index.push(42 / arg.value);
        Ok(())
    }
}

impl Service for TestServiceImpl {}

impl_service_dispatcher!(TestServiceImpl, TestService);

impl ServiceFactory for TestServiceImpl {
    fn artifact(&self) -> RustArtifactSpec {
        RustArtifactSpec::new("test_service", 1, 0, 0)
    }

    fn new_instance(&self) -> Box<dyn Service> {
        Box::new(TestServiceImpl)
    }
}

#[derive(Serialize, Deserialize, ProtobufConvert, Debug, Clone)]
#[exonum(pb = "TestServiceTx", crate = "crate")]
struct Tx {
    value: u64,
}

impl Tx {
    fn new(value: u64) -> Self {
        Self { value }
    }
}

// fn handling_tx_panic_storage_error(blockchain: &mut Blockchain) {
//     let (pk, sec_key) = gen_keypair();
//     let tx_ok1 = Message::sign_transaction(Tx::new(3), TEST_SERVICE_ID, pk, &sec_key);
//     let tx_ok2 = Message::sign_transaction(Tx::new(4), TEST_SERVICE_ID, pk, &sec_key);
//     let tx_failed = Message::sign_transaction(Tx::new(0), TEST_SERVICE_ID, pk, &sec_key);
//     let tx_storage_error = Message::sign_transaction(Tx::new(42), TEST_SERVICE_ID, pk, &sec_key);

//     let patch = {
//         let fork = blockchain.fork();
//         {
//             let mut schema = Schema::new(&fork);
//             schema.add_transaction_into_pool(tx_ok1.clone());
//             schema.add_transaction_into_pool(tx_ok2.clone());
//             schema.add_transaction_into_pool(tx_failed.clone());
//             schema.add_transaction_into_pool(tx_storage_error.clone());
//         }
//         fork.into_patch()
//     };
//     blockchain.merge(patch).unwrap();
//     blockchain.create_patch(
//         ValidatorId::zero(),
//         Height::zero(),
//         &[
//             tx_ok1.object_hash(),
//             tx_storage_error.object_hash(),
//             tx_ok2.object_hash(),
//         ],
//     );
// }

// struct ServiceGood;

// impl Service for ServiceGood {
//     fn service_id(&self) -> u16 {
//         1
//     }
//     fn service_name(&self) -> &'static str {
//         "some_service"
//     }

//     fn state_hash(&self, _snapshot: &dyn Snapshot) -> Vec<Hash> {
//         vec![]
//     }

//     fn tx_from_raw(&self, _raw: AnyTx) -> Result<Box<dyn Transaction>, failure::Error> {
//         unimplemented!()
//     }

//     fn before_commit(&self, fork: &Fork) {
//         let mut index = ListIndex::new(IDX_NAME, fork);
//         index.push(1);
//     }
// }

// struct ServicePanic;

// impl Service for ServicePanic {
//     fn service_id(&self) -> u16 {
//         1
//     }

//     fn service_name(&self) -> &'static str {
//         "some_service"
//     }

//     fn state_hash(&self, _snapshot: &dyn Snapshot) -> Vec<Hash> {
//         vec![]
//     }

//     fn tx_from_raw(&self, _raw: AnyTx) -> Result<Box<dyn Transaction>, failure::Error> {
//         unimplemented!()
//     }

//     fn before_commit(&self, _fork: &Fork) {
//         panic!("42");
//     }
// }

// struct ServicePanicStorageError;

// impl Service for ServicePanicStorageError {
//     fn service_id(&self) -> u16 {
//         1
//     }

//     fn service_name(&self) -> &'static str {
//         "some_service"
//     }

//     fn state_hash(&self, _snapshot: &dyn Snapshot) -> Vec<Hash> {
//         vec![]
//     }

//     fn tx_from_raw(&self, _raw: AnyTx) -> Result<Box<dyn Transaction>, failure::Error> {
//         unimplemented!()
//     }

//     fn before_commit(&self, _fork: &Fork) {
//         panic!(ExecutionError::new(42));
//     }
// }

// fn assert_service_execute(blockchain: &Blockchain, db: &mut dyn Database) {
//     let (_, patch) = blockchain.create_patch(ValidatorId::zero(), Height(1), &[]);
//     db.merge(patch).unwrap();
//     let snapshot = db.snapshot();
//     let index = ListIndex::new(IDX_NAME, &snapshot);
//     assert_eq!(index.len(), 1);
//     assert_eq!(index.get(0), Some(1));
// }

// fn assert_service_execute_panic(blockchain: &Blockchain, db: &mut dyn Database) {
//     let (_, patch) = blockchain.create_patch(ValidatorId::zero(), Height(1), &[]);
//     db.merge(patch).unwrap();
//     let snapshot = db.snapshot();
//     let index: ListIndex<_, u32> = ListIndex::new(IDX_NAME, &snapshot);
//     assert!(index.is_empty());
// }

fn create_blockchain() -> Blockchain {
    let service_keypair = gen_keypair();
    let api_channel = mpsc::unbounded();
    let internal_sender = mpsc::channel(1).0;

    Blockchain::with_dispatcher(
        TemporaryDB::new(),
        DispatcherBuilder::new(internal_sender)
            .with_builtin_service(TestServiceImpl, TEST_SERVICE_ID, IDX_NAME)
            .finalize(),
        service_keypair.0,
        service_keypair.1,
        ApiSender::new(api_channel.0),
    )
}

// fn create_blockchain_with_service(path: &Path, service: Box<dyn Service>) -> Blockchain {
//     let db = create_database(path);
//     let service_keypair = gen_keypair();
//     let api_channel = mpsc::unbounded();
//     let internal_sender = mpsc::channel(1).0;

//     Blockchain::new(
//         db,
//         //            vec![service],
//         Vec::new(), // TODO: use new service API.
//         service_keypair.0,
//         service_keypair.1,
//         ApiSender::new(api_channel.0),
//         internal_sender,
//     )
// }

#[test]
fn handling_tx_panic() {
    let mut blockchain = create_blockchain();

    let (pk, sec_key) = gen_keypair();
    let tx_ok1 = Message::sign_transaction(Tx::new(3), TEST_SERVICE_ID, pk, &sec_key);
    let tx_ok2 = Message::sign_transaction(Tx::new(4), TEST_SERVICE_ID, pk, &sec_key);
    let tx_failed = Message::sign_transaction(Tx::new(0), TEST_SERVICE_ID, pk, &sec_key);
    let tx_storage_error = Message::sign_transaction(Tx::new(42), TEST_SERVICE_ID, pk, &sec_key);

    let patch = {
        let fork = blockchain.fork();
        {
            let mut schema = Schema::new(&fork);

            schema.add_transaction_into_pool(tx_ok1.clone());
            schema.add_transaction_into_pool(tx_ok2.clone());
            schema.add_transaction_into_pool(tx_failed.clone());
            schema.add_transaction_into_pool(tx_storage_error.clone());
        }
        fork.into_patch()
    };
    blockchain.merge(patch).unwrap();

    let (_, patch) = blockchain.create_patch(
        ValidatorId::zero(),
        Height::zero(),
        &[
            tx_ok1.object_hash(),
            tx_failed.object_hash(),
            tx_ok2.object_hash(),
        ],
    );

    blockchain.merge(patch).unwrap();
    let snapshot = blockchain.snapshot();

    let schema = Schema::new(&snapshot);
    assert_eq!(
        schema.transactions().get(&tx_ok1.object_hash()),
        Some(tx_ok1.clone())
    );
    assert_eq!(
        schema.transactions().get(&tx_ok2.object_hash()),
        Some(tx_ok2.clone())
    );
    assert_eq!(
        schema.transactions().get(&tx_failed.object_hash()),
        Some(tx_failed.clone())
    );

    let index = ListIndex::new(IDX_NAME, &snapshot);

    assert_eq!(index.len(), 4);
    assert_eq!(index.get(0), Some(3));
    assert_eq!(index.get(1), Some(14));
    assert_eq!(index.get(2), Some(4));
    assert_eq!(index.get(3), Some(10));
}

// #[test]
// #[should_panic]
// fn handling_tx_panic_storage_error() {
//     let dir = create_temp_dir();
//     let mut blockchain = create_blockchain(dir.path());
//     super::handling_tx_panic_storage_error(&mut blockchain);
// }

// #[test]
// fn service_execute() {
//     let dir = create_temp_dir();
//     let blockchain = create_blockchain_with_service(dir.path(), Box::new(ServiceGood));
//     let dir = create_temp_dir();
//     let mut db = create_database(dir.path());
//     super::assert_service_execute(&blockchain, db.as_mut());
// }

// #[test]
// fn service_execute_panic() {
//     let dir = create_temp_dir();
//     let blockchain = create_blockchain_with_service(dir.path(), Box::new(ServicePanic));
//     let dir = create_temp_dir();
//     let mut db = create_database(dir.path());
//     super::assert_service_execute_panic(&blockchain, db.as_mut());
// }

// #[test]
// #[should_panic]
// fn service_execute_panic_storage_error() {
//     let dir = create_temp_dir();
//     let blockchain = create_blockchain_with_service(dir.path(), Box::new(ServicePanicStorageError));
//     let dir = create_temp_dir();
//     let mut db = create_database(dir.path());
//     super::assert_service_execute(&blockchain, db.as_mut());
// }

mod transactions_tests {
    use serde::{de::DeserializeOwned, Serialize};

    use crate::{
        blockchain::{ExecutionResult, Transaction, TransactionContext, TransactionSet},
        crypto::gen_keypair,
        messages::{AnyTx, BinaryValue, Message, ServiceTransaction, Signed},
        proto::schema::tests::{BlockchainTestTxA, BlockchainTestTxB},
    };

    #[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert)]
    #[exonum(pb = "BlockchainTestTxA", crate = "crate")]
    struct A {
        a: u64,
    }

    impl A {
        fn new(a: u64) -> Self {
            Self { a }
        }
    }

    #[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert)]
    #[exonum(pb = "BlockchainTestTxB", crate = "crate")]
    struct B {
        b: u64,
        c: u32,
    }

    impl B {
        fn new(b: u64, c: u32) -> Self {
            Self { b, c }
        }
    }

    #[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert)]
    #[exonum(pb = "BlockchainTestTxA", crate = "crate")]
    struct C {
        a: u64,
    }

    impl C {
        fn new(a: u64) -> Self {
            Self { a }
        }
    }

    #[derive(Serialize, Deserialize, Clone, Debug, TransactionSet)]
    #[exonum(crate = "crate")]
    enum MyTransactions {
        A(A),
        B(B),
        C(C),
    }

    impl Transaction for A {
        fn execute(&self, _: TransactionContext) -> ExecutionResult {
            Ok(())
        }
    }

    impl Transaction for B {
        fn execute(&self, _: TransactionContext) -> ExecutionResult {
            Ok(())
        }
    }

    impl Transaction for C {
        fn execute(&self, _: TransactionContext) -> ExecutionResult {
            Ok(())
        }
    }

    // TODO we should rewrite this test ECR-3222
    // #[test]
    // fn deserialize_from_raw() {
    //     fn round_trip<T>(t: T)
    //         where T: Into<ServiceTransaction>
    //     {
    //         let (pk, sec_key) = gen_keypair();
    //         let set = t.into();
    //         let initial_json = serde_json::to_value(&set).unwrap();

    //         let signed = Message::sign_transaction(set, 0, pk, &sec_key);
    //         let parsed = Signed::<AnyTx>::from_bytes(signed.to_bytes().into()).unwrap();
    //         let round_tripped = serde_json::to_value(&parsed).unwrap();
    //         assert_eq!(initial_json, round_tripped);
    //     }

    //     let a = A::new(0);
    //     let b = B::new(1, 2);
    //     let c = C::new(0);
    //     round_trip(a);
    //     round_trip(b);
    //     round_trip(c);
    // }
}
