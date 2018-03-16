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

#![allow(dead_code, unasfe_code)]

use std::collections::BTreeMap;

use rand::{thread_rng, Rng};
use serde_json;

use blockchain::{Blockchain, Schema, Transaction, ExecutionResult};
use crypto::{gen_keypair, CryptoHash, Hash};
use storage::{Database, Fork, Error, ListIndex};
use messages::Message;
use helpers::{Height, ValidatorId};

const IDX_NAME: &'static str = "idx_name";

#[test]
fn test_encode_decode() {
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
fn test_u64() {
    encoding_struct! {
        struct Test {
            some_test: u64,
        }
    }
    let test_data = r##"{"some_test":"1234"}"##;
    let test = Test::new(1234);
    let data = ::serde_json::to_string(&test).unwrap();
    assert_eq!(data, test_data);
}

#[test]
fn test_system_time() {
    use std::time::{SystemTime, UNIX_EPOCH};
    encoding_struct! {
        struct Test {
            some_test: SystemTime,
        }
    }
    let test_data = r##"{"some_test":{"nanos":0,"secs":"0"}}"##;


    let test = Test::new(UNIX_EPOCH);
    let data = ::serde_json::to_string(&test).unwrap();
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
fn test_correct_encoding_struct() {
    let dat: Vec<u8> =
        vec![8u8, 0, 0, 0, 18, 0, 0, 0, 16, 0, 0, 0, 1, 0, 0, 0, 17, 0, 0, 0, 1, 0, 0, 0, 1, 2];
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
fn test_overlap_segments() {
    let test = vec![16u8, 0, 0, 0, 1, 0, 0, 0, 16, 0, 0, 0, 1, 0, 0, 0, 1, 2];
    let mut buffer = vec![0; 8];
    test.write(&mut buffer, 0, 8);
    <StructWithTwoSegments as Field>::check(&buffer, 0.into(), 8.into(), 8.into()).unwrap();
}


#[test]
#[should_panic(expected = "SpaceBetweenSegments")]
fn test_segments_has_spaces_between() {
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
    thread_rng().gen_ascii_chars().take(10).collect()
}

fn handling_tx_panic(blockchain: &Blockchain, db: &mut Box<Database>) {
    messages! {
        const SERVICE_ID = 1;
        struct Tx {
            value: u64,
        }
    }

    impl Transaction for Tx {
        fn verify(&self) -> bool {
            true
        }

        fn execute(&self, fork: &mut Fork) -> ExecutionResult {
            if self.value() == 42 {
                panic!(Error::new("42"))
            }

            let mut index = ListIndex::new(IDX_NAME, fork);
            index.push(self.value());
            index.push(42 / self.value());

            Ok(())
        }
    }

    let (_, sec_key) = gen_keypair();

    let tx_ok1 = Tx::new(3, &sec_key);
    let tx_ok2 = Tx::new(4, &sec_key);
    let tx_failed = Tx::new(0, &sec_key);
    let tx_storage_error = Tx::new(42, &sec_key);

    let mut pool: BTreeMap<Hash, Box<Transaction>> = BTreeMap::new();

    pool.insert(tx_ok1.hash(), Box::new(tx_ok1.clone()) as Box<Transaction>);
    pool.insert(tx_ok2.hash(), Box::new(tx_ok2.clone()) as Box<Transaction>);
    pool.insert(
        tx_failed.hash(),
        Box::new(tx_failed.clone()) as Box<Transaction>,
    );
    pool.insert(
        tx_storage_error.hash(),
        Box::new(tx_storage_error.clone()) as Box<Transaction>,
    );

    let (_, patch) = blockchain.create_patch(
        ValidatorId::zero(),
        Height::zero(),
        &[tx_ok1.hash(), tx_failed.hash(), tx_ok2.hash()],
        &pool,
    );

    db.merge(patch).unwrap();
    let snapshot = db.snapshot();

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

fn handling_tx_panic_storage_error(blockchain: &Blockchain) {
    messages! {
        const SERVICE_ID = 1;
        struct Tx {
            value: u64,
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

    let (_, sec_key) = gen_keypair();

    let tx_ok1 = Tx::new(3, &sec_key);
    let tx_ok2 = Tx::new(4, &sec_key);
    let tx_failed = Tx::new(0, &sec_key);
    let tx_storage_error = Tx::new(42, &sec_key);

    let mut pool: BTreeMap<Hash, Box<Transaction>> = BTreeMap::new();

    pool.insert(tx_ok1.hash(), Box::new(tx_ok1.clone()) as Box<Transaction>);
    pool.insert(tx_ok2.hash(), Box::new(tx_ok2.clone()) as Box<Transaction>);
    pool.insert(
        tx_failed.hash(),
        Box::new(tx_failed.clone()) as Box<Transaction>,
    );
    pool.insert(
        tx_storage_error.hash(),
        Box::new(tx_storage_error.clone()) as Box<Transaction>,
    );

    blockchain.create_patch(
        ValidatorId::zero(),
        Height::zero(),
        &[tx_ok1.hash(), tx_storage_error.hash(), tx_ok2.hash()],
        &pool,
    );
}

mod transactions_tests {
    use blockchain::{Transaction, TransactionSet, ExecutionResult};
    use storage::Fork;
    use crypto::gen_keypair;
    use serde::Serialize;
    use serde_json;

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

mod memorydb_tests {
    use futures::sync::mpsc;
    use std::path::Path;
    use tempdir::TempDir;
    use storage::{Database, MemoryDB};
    use blockchain::Blockchain;
    use crypto::gen_keypair;
    use node::ApiSender;

    fn create_database(_: &Path) -> Box<Database> {
        Box::new(MemoryDB::new())
    }

    fn create_blockchain(_: &Path) -> Blockchain {
        let service_keypair = gen_keypair();
        let api_channel = mpsc::channel(1);
        Blockchain::new(
            MemoryDB::new(),
            Vec::new(),
            service_keypair.0,
            service_keypair.1,
            ApiSender::new(api_channel.0),
        )
    }

    #[test]
    fn test_handling_tx_panic() {
        let dir = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path = dir.path();
        let blockchain = create_blockchain(path);
        let dir1 = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path1 = dir1.path();
        let mut db = create_database(path1);
        super::handling_tx_panic(&blockchain, &mut db);
    }

    #[test]
    #[should_panic]
    fn test_handling_tx_panic_storage_error() {
        let dir = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path = dir.path();
        let blockchain = create_blockchain(path);
        super::handling_tx_panic_storage_error(&blockchain);
    }
}

mod rocksdb_tests {
    use futures::sync::mpsc;
    use std::path::Path;
    use tempdir::TempDir;
    use storage::{Database, RocksDB, DbOptions};
    use blockchain::Blockchain;
    use crypto::gen_keypair;
    use node::ApiSender;

    fn create_database(path: &Path) -> Box<Database> {
        let opts = DbOptions::default();
        Box::new(RocksDB::open(path, &opts).unwrap())
    }

    fn create_blockchain(path: &Path) -> Blockchain {
        let db = create_database(path);
        let service_keypair = gen_keypair();
        let api_channel = mpsc::channel(1);
        Blockchain::new(
            db,
            Vec::new(),
            service_keypair.0,
            service_keypair.1,
            ApiSender::new(api_channel.0),
        )
    }

    #[test]
    fn test_handling_tx_panic() {
        let dir = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path = dir.path();
        let blockchain = create_blockchain(path);
        let dir1 = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path1 = dir1.path();
        let mut db = create_database(path1);
        super::handling_tx_panic(&blockchain, &mut db);
    }

    #[test]
    #[should_panic]
    fn test_handling_tx_panic_storage_error() {
        let dir = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
        let path = dir.path();
        let blockchain = create_blockchain(path);
        super::handling_tx_panic_storage_error(&blockchain);
    }
}
