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

use chrono::{DateTime, TimeZone, Utc};
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use serde_json;

use blockchain::{Blockchain, ExecutionResult, Schema, Service, Transaction};
use crypto::{gen_keypair, CryptoHash, Hash};
use encoding::Error as MessageError;
use helpers::{Height, ValidatorId};
use messages::{Message, RawTransaction};
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

    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<dyn Transaction>, MessageError> {
        Ok(Box::new(Tx::from_raw(raw)?))
    }
}

transactions! {
    TestServiceTxs {
        const SERVICE_ID = TEST_SERVICE_ID;
        struct Tx {
            value: u64,
        }
    }
}

impl Transaction for Tx {
    fn verify(&self) -> bool {
        true
    }

    fn execute(&self, view: &mut Fork) -> ExecutionResult {
        if self.value() == 42 {
            panic!(Error::new("42"))
        }
        let mut index = ListIndex::new(IDX_NAME, view);
        index.push(self.value());
        index.push(42 / self.value());
        Ok(())
    }
}

#[test]
fn encode_decode() {
    encoding_struct! {
        struct Parent {
            child: Child,
        }
    }

    encoding_struct! {
        struct Child {
            child: &Hash,
        }
    }
    let content = Child::new(&Hash::zero());
    let par = Parent::new(content);
    let par_json = serde_json::to_value(par.clone()).unwrap();
    assert_eq!(serde_json::from_value::<Parent>(par_json).unwrap(), par);
}

#[test]
fn u64_json_serialization() {
    encoding_struct! {
        struct Test {
            some_test: u64,
        }
    }
    let test_data = r##"{"some_test":"1234"}"##;
    let test = Test::new(1234);
    let data = serde_json::to_string(&test).unwrap();
    assert_eq!(data, test_data);
}

#[test]
fn date_time_json_serialization() {
    encoding_struct! {
        struct Test {
            some_test: DateTime<Utc>,
        }
    }

    let test_data = r##"{"some_test":{"nanos":0,"secs":"0"}}"##;

    let test = Test::new(Utc.timestamp(0, 0));
    let data = serde_json::to_string(&test).unwrap();
    assert_eq!(data, test_data);
}

use encoding::Field;

encoding_struct! {
    struct StructWithTwoSegments {
        first: &[u8],
        second: &[u8],
    }
}

#[test]
fn correct_encoding_struct() {
    let dat: Vec<u8> = vec![
        8u8, 0, 0, 0, 18, 0, 0, 0, 16, 0, 0, 0, 1, 0, 0, 0, 17, 0, 0, 0, 1, 0, 0, 0, 1, 2,
    ];
    let test = vec![16u8, 0, 0, 0, 1, 0, 0, 0, 17, 0, 0, 0, 1, 0, 0, 0, 1, 2];
    let mut buffer = vec![0; 8];
    test.write(&mut buffer, 0, 8);
    assert_eq!(buffer, dat);
    <StructWithTwoSegments as Field>::check(&dat, 0.into(), 8.into(), 8.into()).unwrap();
    let struct_ = unsafe { <StructWithTwoSegments as Field>::read(&dat, 0, 8) };
    assert_eq!(struct_.first(), &[1u8]);
    assert_eq!(struct_.second(), &[2u8]);
}

#[test]
#[should_panic(expected = "OverlappingSegment")]
fn overlap_segments() {
    let test = vec![16u8, 0, 0, 0, 1, 0, 0, 0, 16, 0, 0, 0, 1, 0, 0, 0, 1, 2];
    let mut buffer = vec![0; 8];
    test.write(&mut buffer, 0, 8);
    <StructWithTwoSegments as Field>::check(&buffer, 0.into(), 8.into(), 8.into()).unwrap();
}

#[test]
#[should_panic(expected = "SpaceBetweenSegments")]
fn segments_has_spaces_between() {
    let test = vec![
        16u8,
        0,
        0,
        0,
        1,
        0,
        0,
        0,
        18,
        0,
        0,
        0,
        1,
        0,
        0,
        0, // <-- link after space
        1,
        0, // <-- this is space one
        2,
    ];
    let mut buffer = vec![0; 8];
    test.write(&mut buffer, 0, 8);
    <StructWithTwoSegments as Field>::check(&buffer, 0.into(), 8.into(), 8.into()).unwrap();
}

fn gen_tempdir_name() -> String {
    thread_rng().sample_iter(&Alphanumeric).take(10).collect()
}

fn handling_tx_panic(blockchain: &mut Blockchain) {
    let (_, sec_key) = gen_keypair();

    let tx_ok1 = Tx::new(3, &sec_key);
    let tx_ok2 = Tx::new(4, &sec_key);
    let tx_failed = Tx::new(0, &sec_key);
    let tx_storage_error = Tx::new(42, &sec_key);

    let patch = {
        let mut fork = blockchain.fork();
        {
            let mut schema = Schema::new(&mut fork);

            schema.add_transaction_into_pool(tx_ok1.raw().clone());
            schema.add_transaction_into_pool(tx_ok2.raw().clone());
            schema.add_transaction_into_pool(tx_failed.raw().clone());
            schema.add_transaction_into_pool(tx_storage_error.raw().clone());
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
        Some(tx_ok1.raw().clone())
    );
    assert_eq!(
        schema.transactions().get(&tx_ok2.hash()),
        Some(tx_ok2.raw().clone())
    );
    assert_eq!(
        schema.transactions().get(&tx_failed.hash()),
        Some(tx_failed.raw().clone())
    );

    let index = ListIndex::new(IDX_NAME, &snapshot);

    assert_eq!(index.len(), 4);
    assert_eq!(index.get(0), Some(3));
    assert_eq!(index.get(1), Some(14));
    assert_eq!(index.get(2), Some(4));
    assert_eq!(index.get(3), Some(10));
}

fn handling_tx_panic_storage_error(blockchain: &mut Blockchain) {
    let (_, sec_key) = gen_keypair();

    let tx_ok1 = Tx::new(3, &sec_key);
    let tx_ok2 = Tx::new(4, &sec_key);
    let tx_failed = Tx::new(0, &sec_key);
    let tx_storage_error = Tx::new(42, &sec_key);

    let patch = {
        let mut fork = blockchain.fork();
        {
            let mut schema = Schema::new(&mut fork);
            schema.add_transaction_into_pool(tx_ok1.raw().clone());
            schema.add_transaction_into_pool(tx_ok2.raw().clone());
            schema.add_transaction_into_pool(tx_failed.raw().clone());
            schema.add_transaction_into_pool(tx_storage_error.raw().clone());
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
    use blockchain::{ExecutionResult, Transaction, TransactionSet};
    use crypto::gen_keypair;
    use serde::Serialize;
    use serde_json;
    use storage::Fork;

    transactions! {
        MyTransactions {
            const SERVICE_ID = 92;

            struct A {
                a: u32
            }

            struct B {
                b: u32,
                c: u8
            }

            struct C {
                a: u32
            }
        }
    }

    impl Transaction for A {
        fn verify(&self) -> bool {
            true
        }

        fn execute(&self, _: &mut Fork) -> ExecutionResult {
            Ok(())
        }
    }

    impl Transaction for B {
        fn verify(&self) -> bool {
            true
        }

        fn execute(&self, _: &mut Fork) -> ExecutionResult {
            Ok(())
        }
    }

    impl Transaction for C {
        fn verify(&self) -> bool {
            true
        }

        fn execute(&self, _: &mut Fork) -> ExecutionResult {
            Ok(())
        }
    }

    #[test]
    fn deserialize_from_json() {
        fn round_trip<T: Transaction + Serialize>(t: &T) {
            let initial = serde_json::to_value(&t).unwrap();
            let parsed: MyTransactions = serde_json::from_value(initial.clone()).unwrap();
            let round_tripped = serde_json::to_value(&parsed).unwrap();
            assert_eq!(initial, round_tripped);
        }

        let (_pub_key, sec_key) = gen_keypair();
        let a = A::new(0, &sec_key);
        let b = B::new(1, 2, &sec_key);
        let c = C::new(0, &sec_key);
        round_trip(&a);
        round_trip(&b);
        round_trip(&c);
    }

    #[test]
    fn deserialize_from_raw() {
        fn round_trip<T: Transaction + Serialize>(t: &T) {
            let initial = serde_json::to_value(&t).unwrap();
            let raw = t.raw();
            let parsed: MyTransactions = TransactionSet::tx_from_raw(raw.clone()).unwrap();
            let round_tripped = serde_json::to_value(&parsed).unwrap();
            assert_eq!(initial, round_tripped);
        }

        let (_pub_key, sec_key) = gen_keypair();
        let a = A::new(0, &sec_key);
        let b = B::new(1, 2, &sec_key);
        let c = C::new(0, &sec_key);
        round_trip(&a);
        round_trip(&b);
        round_trip(&c);
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

    fn tx_from_raw(&self, _raw: RawTransaction) -> Result<Box<dyn Transaction>, MessageError> {
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

    fn tx_from_raw(&self, _raw: RawTransaction) -> Result<Box<dyn Transaction>, MessageError> {
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

    fn tx_from_raw(&self, _raw: RawTransaction) -> Result<Box<dyn Transaction>, MessageError> {
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
