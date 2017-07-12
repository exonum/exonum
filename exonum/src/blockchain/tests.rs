#![allow(dead_code)]

#[test]
fn test_u64() {
    encoding_struct! {
        struct Test {
            const SIZE = 8;
            field some_test:u64 [0 => 8]
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
            const SIZE = 12;
            field some_test:SystemTime [0 => 12]
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
        const SIZE = 16;
        field first:  &[u8]     [0 => 8]
        field second: &[u8]     [8 => 16]
    }
}

#[test]
fn test_correct_encoding_struct() {
    let dat: Vec<u8> = vec![8u8, 0, 0, 0, 18, 0, 0, 0, 16, 0, 0, 0, 1, 0, 0, 0, 17, 0, 0, 0, 1, 0,
                            0, 0, 1, 2];
    let test = vec![16u8, 0, 0, 0, 1, 0, 0, 0, 17, 0, 0, 0, 1, 0, 0, 0, 1, 2];
    let mut buffer = vec![0;8];
    test.write(&mut buffer, 0, 8);
    assert_eq!(buffer, dat);
    <StructWithTwoSegments as Field>::check(&dat, 0.into(), 8.into(), 8.into()).unwrap();
    let strukt = unsafe { <StructWithTwoSegments as Field>::read(&dat, 0, 8) };
    assert_eq!(strukt.first(), &[1u8]);
    assert_eq!(strukt.second(), &[2u8]);
}

#[test]
#[should_panic(expected ="OverlappingSegment")]
fn test_overlap_segments() {
    let test = vec![16u8, 0, 0, 0, 1, 0, 0, 0, 16, 0, 0, 0, 1, 0, 0, 0, 1, 2];
    let mut buffer = vec![0;8];
    test.write(&mut buffer, 0, 8);
    <StructWithTwoSegments as Field>::check(&buffer, 0.into(), 8.into(), 8.into()).unwrap();
}


#[test]
#[should_panic(expected ="SpaceBetweenSegments")]
fn test_segments_has_spaces_between() {
    let test = vec![16u8, 0, 0, 0, 1, 0, 0, 0,
        18, 0, 0, 0, 1, 0, 0, 0, // <-- link after space
        1,
        0, // <-- this is space one
        2];
    let mut buffer = vec![0;8];
    test.write(&mut buffer, 0, 8);
    <StructWithTwoSegments as Field>::check(&buffer, 0.into(), 8.into(), 8.into()).unwrap();
}


#[test]
fn test_handling_tx_panic() {
    use std::collections::BTreeMap;
    use super::{Blockchain, Schema, Transaction};
    use crypto::{gen_keypair, Hash};
    use storage::{Database, MemoryDB, Fork, Error, ListIndex};
    use messages::Message;

    let blockchain = Blockchain::new(Box::new(MemoryDB::new()), Vec::new());

    message! {
        struct Tx {
            const TYPE = 1;
            const ID = 0;
            const SIZE = 8;

            field value: u64 [0 => 8]
        }
    }

    impl Transaction for Tx {
        fn verify(&self) -> bool {
            true
        }

        fn execute(&self, view: &mut Fork) {
            if self.value() == 42 {
                panic!(Error::new("42"))
            }
            let mut index = ListIndex::new(vec![01], view);
            index.push(self.value());
            index.push(42 / self.value());
        }
    }

    let (_, sec_key) = gen_keypair();

    let tx_ok1 = Tx::new(3, &sec_key);
    let tx_ok2 = Tx::new(4, &sec_key);
    let tx_failed = Tx::new(0, &sec_key);
    let tx_storage_error = Tx::new(42, &sec_key);

    let mut pool : BTreeMap<Hash, Box<Transaction>> = BTreeMap::new();

    pool.insert(tx_ok1.hash(), Box::new(tx_ok1.clone()) as Box<Transaction>);
    pool.insert(tx_ok2.hash(), Box::new(tx_ok2.clone()) as Box<Transaction>);
    pool.insert(tx_failed.hash(), Box::new(tx_failed.clone()) as Box<Transaction>);
    pool.insert(tx_storage_error.hash(), Box::new(tx_storage_error.clone()) as Box<Transaction>);

    let (_, patch) = blockchain.create_patch(0, 0, &[tx_ok1.hash(), tx_failed.hash(), tx_ok2.hash()], &pool);

    let mut db = MemoryDB::new();
    db.merge(patch).unwrap();
    let snapshot = db.snapshot();

    let schema = Schema::new(&snapshot);

    assert_eq!(schema.transactions().get(&tx_ok1.hash()), Some(tx_ok1.raw().clone()));
    assert_eq!(schema.transactions().get(&tx_ok2.hash()), Some(tx_ok2.raw().clone()));
    assert_eq!(schema.transactions().get(&tx_failed.hash()), Some(tx_failed.raw().clone()));

    let index = ListIndex::new(vec![01], &snapshot);

    assert_eq!(index.len(), 4);
    assert_eq!(index.get(0), Some(3));
    assert_eq!(index.get(1), Some(14));
    assert_eq!(index.get(2), Some(4));
    assert_eq!(index.get(3), Some(10));
}


#[test]
#[should_panic]
fn test_handling_tx_panic_storage_error() {
    use std::collections::BTreeMap;
    use super::{Blockchain, Transaction};
    use crypto::{gen_keypair, Hash};
    use storage::{MemoryDB, Fork, Error, ListIndex};
    use messages::Message;

    let blockchain = Blockchain::new(Box::new(MemoryDB::new()), Vec::new());

    message! {
        struct Tx {
            const TYPE = 1;
            const ID = 0;
            const SIZE = 8;

            field value: u64 [0 => 8]
        }
    }

    impl Transaction for Tx {
        fn verify(&self) -> bool {
            true
        }

        fn execute(&self, view: &mut Fork) {
            if self.value() == 42 {
                panic!(Error::new("42"))
            }
            let mut index = ListIndex::new(vec![01], view);
            index.push(self.value());
            index.push(42 / self.value());
        }
    }

    let (_, sec_key) = gen_keypair();

    let tx_ok1 = Tx::new(3, &sec_key);
    let tx_ok2 = Tx::new(4, &sec_key);
    let tx_failed = Tx::new(0, &sec_key);
    let tx_storage_error = Tx::new(42, &sec_key);

    let mut pool : BTreeMap<Hash, Box<Transaction>> = BTreeMap::new();

    pool.insert(tx_ok1.hash(), Box::new(tx_ok1.clone()) as Box<Transaction>);
    pool.insert(tx_ok2.hash(), Box::new(tx_ok2.clone()) as Box<Transaction>);
    pool.insert(tx_failed.hash(), Box::new(tx_failed.clone()) as Box<Transaction>);
    pool.insert(tx_storage_error.hash(), Box::new(tx_storage_error.clone()) as Box<Transaction>);

    blockchain.create_patch(0, 0, &[tx_ok1.hash(), tx_storage_error.hash(), tx_ok2.hash()], &pool);
}
