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

#![allow(dead_code, unsafe_code)]

use rand::{distributions::Alphanumeric, thread_rng, Rng};

use blockchain::{
    Blockchain, ExecutionResult, Schema, Service, Transaction, TransactionContext, TransactionSet,
};
use crypto::{gen_keypair, Hash};
use helpers::{Height, ValidatorId};
use messages::{Message, RawTransaction};
use proto;
use storage::{Database, Error, Fork, ListIndex, Snapshot};

const IDX_NAME: &'static str = "idx_name";
const TEST_SERVICE_ID: u16 = 255;

struct TestService;

impl Service for TestService {
    fn service_id(&self) -> u16 {
        TEST_SERVICE_ID
    }

    fn service_name(&self) -> &'static str {
        "test service"
    }

    fn state_hash(&self, _: &dyn Snapshot) -> Vec<Hash> {
        vec![]
    }

    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<dyn Transaction>, failure::Error> {
        Ok(TestServiceTxs::tx_from_raw(raw)?.into())
    }
}

#[derive(Serialize, Deserialize, ProtobufConvert, Debug, Clone)]
#[exonum(pb = "proto::schema::tests::TestServiceTx", crate = "crate")]
struct Tx {
    value: u64,
}

impl Tx {
    fn new(value: u64) -> Self {
        Self { value }
    }
}

#[derive(Serialize, Deserialize, Clone, TransactionSet, Debug)]
#[exonum(crate = "crate")]
enum TestServiceTxs {
    Tx(Tx),
}

impl Transaction for Tx {
    fn execute(&self, mut tc: TransactionContext) -> ExecutionResult {
        if self.value == 42 {
            panic!(Error::new("42"))
        }
        let mut index = ListIndex::new(IDX_NAME, tc.fork());
        index.push(self.value);
        index.push(42 / self.value);
        Ok(())
    }
}

fn gen_tempdir_name() -> String {
    thread_rng().sample_iter(&Alphanumeric).take(10).collect()
}

fn handling_tx_panic(blockchain: &mut Blockchain) {
    let (pk, sec_key) = gen_keypair();
    let tx_ok1 = Message::sign_transaction(Tx::new(3), TEST_SERVICE_ID, pk, &sec_key);
    let tx_ok2 = Message::sign_transaction(Tx::new(4), TEST_SERVICE_ID, pk, &sec_key);
    let tx_failed = Message::sign_transaction(Tx::new(0), TEST_SERVICE_ID, pk, &sec_key);
    let tx_storage_error = Message::sign_transaction(Tx::new(42), TEST_SERVICE_ID, pk, &sec_key);

    let patch = {
        let mut fork = blockchain.fork();
        {
            let mut schema = Schema::new(&mut fork);

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
        &[tx_ok1.hash(), tx_failed.hash(), tx_ok2.hash()],
    );

    blockchain.merge(patch).unwrap();
    let snapshot = blockchain.snapshot();

    let schema = Schema::new(&snapshot);
    assert_eq!(
        schema.transactions().get(&tx_ok1.hash()),
        Some(tx_ok1.clone())
    );
    assert_eq!(
        schema.transactions().get(&tx_ok2.hash()),
        Some(tx_ok2.clone())
    );
    assert_eq!(
        schema.transactions().get(&tx_failed.hash()),
        Some(tx_failed.clone())
    );

    let index = ListIndex::new(IDX_NAME, &snapshot);

    assert_eq!(index.len(), 4);
    assert_eq!(index.get(0), Some(3));
    assert_eq!(index.get(1), Some(14));
    assert_eq!(index.get(2), Some(4));
    assert_eq!(index.get(3), Some(10));
}

fn handling_tx_panic_storage_error(blockchain: &mut Blockchain) {
    let (pk, sec_key) = gen_keypair();
    let tx_ok1 = Message::sign_transaction(Tx::new(3), TEST_SERVICE_ID, pk, &sec_key);
    let tx_ok2 = Message::sign_transaction(Tx::new(4), TEST_SERVICE_ID, pk, &sec_key);
    let tx_failed = Message::sign_transaction(Tx::new(0), TEST_SERVICE_ID, pk, &sec_key);
    let tx_storage_error = Message::sign_transaction(Tx::new(42), TEST_SERVICE_ID, pk, &sec_key);

    let patch = {
        let mut fork = blockchain.fork();
        {
            let mut schema = Schema::new(&mut fork);
            schema.add_transaction_into_pool(tx_ok1.clone());
            schema.add_transaction_into_pool(tx_ok2.clone());
            schema.add_transaction_into_pool(tx_failed.clone());
            schema.add_transaction_into_pool(tx_storage_error.clone());
        }
        fork.into_patch()
    };
    blockchain.merge(patch).unwrap();
    blockchain.create_patch(
        ValidatorId::zero(),
        Height::zero(),
        &[tx_ok1.hash(), tx_storage_error.hash(), tx_ok2.hash()],
    );
}

mod transactions_tests {
    use super::TEST_SERVICE_ID;
    use blockchain::{ExecutionResult, Transaction, TransactionContext};
    use crypto::gen_keypair;
    use messages::Message;
    use serde_json;

    use proto::schema::tests::{BlockchainTestTxA, BlockchainTestTxB};

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

    #[test]
    fn deserialize_from_json() {
        fn round_trip<T: Into<MyTransactions>>(t: T) {
            let t = t.into();
            let initial = serde_json::to_value(&t).unwrap();
            println!("{:?}", initial);
            let parsed: MyTransactions = serde_json::from_value(initial.clone()).unwrap();
            let round_tripped = serde_json::to_value(&parsed).unwrap();
            assert_eq!(initial, round_tripped);
        }

        let a = A::new(0);
        let b = B::new(1, 2);
        let c = C::new(0);
        round_trip(a);
        round_trip(b);
        round_trip(c);
    }

    #[test]
    fn deserialize_from_raw() {
        use blockchain::TransactionSet;

        fn round_trip<T: Into<MyTransactions>>(t: T) {
            let (pk, sec_key) = gen_keypair();
            use std::ops::Deref;
            let set = t.into();
            let initial_json = serde_json::to_value(&set).unwrap();
            let msg = Message::sign_transaction(set, TEST_SERVICE_ID, pk, &sec_key);

            let parsed = MyTransactions::tx_from_raw(msg.deref().clone()).unwrap();
            let round_tripped = serde_json::to_value(&parsed).unwrap();
            assert_eq!(initial_json, round_tripped);
        }

        let a = A::new(0);
        let b = B::new(1, 2);
        let c = C::new(0);
        round_trip(a);
        round_trip(b);
        round_trip(c);
    }
}

struct ServiceGood;

impl Service for ServiceGood {
    fn service_id(&self) -> u16 {
        1
    }
    fn service_name(&self) -> &'static str {
        "some_service"
    }

    fn state_hash(&self, _snapshot: &dyn Snapshot) -> Vec<Hash> {
        vec![]
    }

    fn tx_from_raw(&self, _raw: RawTransaction) -> Result<Box<dyn Transaction>, failure::Error> {
        unimplemented!()
    }

    fn before_commit(&self, fork: &mut Fork) {
        let mut index = ListIndex::new(IDX_NAME, fork);
        index.push(1);
    }
}

struct ServicePanic;

impl Service for ServicePanic {
    fn service_id(&self) -> u16 {
        1
    }

    fn service_name(&self) -> &'static str {
        "some_service"
    }

    fn state_hash(&self, _snapshot: &dyn Snapshot) -> Vec<Hash> {
        vec![]
    }

    fn tx_from_raw(&self, _raw: RawTransaction) -> Result<Box<dyn Transaction>, failure::Error> {
        unimplemented!()
    }

    fn before_commit(&self, _fork: &mut Fork) {
        panic!("42");
    }
}

struct ServicePanicStorageError;

impl Service for ServicePanicStorageError {
    fn service_id(&self) -> u16 {
        1
    }

    fn service_name(&self) -> &'static str {
        "some_service"
    }

    fn state_hash(&self, _snapshot: &dyn Snapshot) -> Vec<Hash> {
        vec![]
    }

    fn tx_from_raw(&self, _raw: RawTransaction) -> Result<Box<dyn Transaction>, failure::Error> {
        unimplemented!()
    }

    fn before_commit(&self, _fork: &mut Fork) {
        panic!(Error::new("42"));
    }
}

fn assert_service_execute(blockchain: &Blockchain, db: &mut Box<dyn Database>) {
    let (_, patch) = blockchain.create_patch(ValidatorId::zero(), Height(1), &[]);
    db.merge(patch).unwrap();
    let snapshot = db.snapshot();
    let index = ListIndex::new(IDX_NAME, &snapshot);
    assert_eq!(index.len(), 1);
    assert_eq!(index.get(0), Some(1));
}

fn assert_service_execute_panic(blockchain: &Blockchain, db: &mut Box<dyn Database>) {
    let (_, patch) = blockchain.create_patch(ValidatorId::zero(), Height(1), &[]);
    db.merge(patch).unwrap();
    let snapshot = db.snapshot();
    let index: ListIndex<_, u32> = ListIndex::new(IDX_NAME, &snapshot);
    assert!(index.is_empty());
}

mod memorydb_tests {
    use blockchain::{Blockchain, Service};
    use crypto::gen_keypair;
    use futures::sync::mpsc;
    use node::ApiSender;
    use storage::{Database, MemoryDB};

    use super::{ServiceGood, ServicePanic, ServicePanicStorageError};

    fn create_database() -> Box<dyn Database> {
        Box::new(MemoryDB::new())
    }

    fn create_blockchain() -> Blockchain {
        let service_keypair = gen_keypair();
        let api_channel = mpsc::channel(1);
        Blockchain::new(
            MemoryDB::new(),
            vec![Box::new(super::TestService) as Box<dyn Service>],
            service_keypair.0,
            service_keypair.1,
            ApiSender::new(api_channel.0),
        )
    }

    fn create_blockchain_with_service(service: Box<dyn Service>) -> Blockchain {
        let service_keypair = gen_keypair();
        let api_channel = mpsc::channel(1);
        Blockchain::new(
            MemoryDB::new(),
            vec![service],
            service_keypair.0,
            service_keypair.1,
            ApiSender::new(api_channel.0),
        )
    }

    #[test]
    fn handling_tx_panic() {
        let mut blockchain = create_blockchain();
        super::handling_tx_panic(&mut blockchain);
    }

    #[test]
    #[should_panic]
    fn handling_tx_panic_storage_error() {
        let mut blockchain = create_blockchain();
        super::handling_tx_panic_storage_error(&mut blockchain);
    }

    #[test]
    fn service_execute() {
        let blockchain = create_blockchain_with_service(Box::new(ServiceGood));
        let mut db = create_database();
        super::assert_service_execute(&blockchain, &mut db);
    }

    #[test]
    fn service_execute_panic() {
        let blockchain = create_blockchain_with_service(Box::new(ServicePanic));
        let mut db = create_database();
        super::assert_service_execute_panic(&blockchain, &mut db);
    }

    #[test]
    #[should_panic]
    fn service_execute_panic_storage_error() {
        let blockchain = create_blockchain_with_service(Box::new(ServicePanicStorageError));
        let mut db = create_database();
        super::assert_service_execute(&blockchain, &mut db);
    }
}

mod rocksdb_tests {
    use blockchain::{Blockchain, Service};
    use crypto::gen_keypair;
    use futures::sync::mpsc;
    use node::ApiSender;
    use std::path::Path;
    use storage::{Database, DbOptions, RocksDB};
    use tempdir::TempDir;

    use super::{ServiceGood, ServicePanic, ServicePanicStorageError};

    fn create_database(path: &Path) -> Box<dyn Database> {
        let opts = DbOptions::default();
        Box::new(RocksDB::open(path, &opts).unwrap())
    }

    fn create_blockchain(path: &Path) -> Blockchain {
        let db = create_database(path);
        let service_keypair = gen_keypair();
        let api_channel = mpsc::channel(1);
        Blockchain::new(
            db,
            vec![Box::new(super::TestService) as Box<dyn Service>],
            service_keypair.0,
            service_keypair.1,
            ApiSender::new(api_channel.0),
        )
    }

    fn create_blockchain_with_service(path: &Path, service: Box<dyn Service>) -> Blockchain {
        let db = create_database(path);
        let service_keypair = gen_keypair();
        let api_channel = mpsc::channel(1);
        Blockchain::new(
            db,
            vec![service],
            service_keypair.0,
            service_keypair.1,
            ApiSender::new(api_channel.0),
        )
    }

    fn create_temp_dir() -> TempDir {
        TempDir::new(super::gen_tempdir_name().as_str()).unwrap()
    }

    #[test]
    fn handling_tx_panic() {
        let dir = create_temp_dir();
        let mut blockchain = create_blockchain(dir.path());
        super::handling_tx_panic(&mut blockchain);
    }

    #[test]
    #[should_panic]
    fn handling_tx_panic_storage_error() {
        let dir = create_temp_dir();
        let mut blockchain = create_blockchain(dir.path());
        super::handling_tx_panic_storage_error(&mut blockchain);
    }

    #[test]
    fn service_execute() {
        let dir = create_temp_dir();
        let blockchain = create_blockchain_with_service(dir.path(), Box::new(ServiceGood));
        let dir = create_temp_dir();
        let mut db = create_database(dir.path());
        super::assert_service_execute(&blockchain, &mut db);
    }

    #[test]
    fn service_execute_panic() {
        let dir = create_temp_dir();
        let blockchain = create_blockchain_with_service(dir.path(), Box::new(ServicePanic));
        let dir = create_temp_dir();
        let mut db = create_database(dir.path());
        super::assert_service_execute_panic(&blockchain, &mut db);
    }

    #[test]
    #[should_panic]
    fn service_execute_panic_storage_error() {
        let dir = create_temp_dir();
        let blockchain =
            create_blockchain_with_service(dir.path(), Box::new(ServicePanicStorageError));
        let dir = create_temp_dir();
        let mut db = create_database(dir.path());
        super::assert_service_execute(&blockchain, &mut db);
    }
}
