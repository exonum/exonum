use std::ops::Deref;

use exonum::blockchain::{Blockchain, Transaction};
use exonum::storage::MemoryDB;
use exonum::crypto::{hash, gen_keypair, Hash, PublicKey, SecretKey};

use exonum_testkit::TestKitBuilder;

use TimestampingService;
use blockchain::dto::{UserInfo, PaymentInfo, Timestamp, TxUpdateUser, TxPayment, TxTimestamp};
use blockchain::schema::{Schema, INITIAL_TIMESTAMPS};
/*
struct TimestampingBlockchain {
    inner: Blockchain,
    payment_keypair: (PublicKey, SecretKey),
}

impl TimestampingBlockchain {
    fn new() -> TimestampingBlockchain {
        let db = MemoryDB::new();
        let timestamping_service = TimestampingService::new();
        let (public_key, private_key) = gen_keypair();
        let inner = Blockchain::new(
            Box::new(db),
            vec![Box::new(timestamping_service)],
            public_key,
            private_key,

        );
        TimestampingBlockchain {
            inner,
            payment_keypair: gen_keypair(),
        }
    }

    fn payment_keypair(&self) -> (PublicKey, SecretKey) {
        self.payment_keypair.clone()
    }

    fn execute_transaction<'a, T>(&mut self, tx: T)
    where
        T: Transaction,
    {
        let mut fork = self.inner.fork();
        tx.execute(&mut fork);
        self.inner.merge(fork.into_patch()).unwrap();
    }
}
impl Deref for TimestampingBlockchain {
    type Target = Blockchain;

    fn deref(&self) -> &Blockchain {
        &self.inner
    }
}
*/

#[test]
fn test_add_user() {
    let mut testkit =TestKitBuilder::validator()
        .with_service(TimestampingService::new())
        .create();
    // Execute transactions
    let keypair = gen_keypair();
    let user_info = UserInfo::new(
        "first_user",
        &keypair.0,
        &keypair.1[..].as_ref(),
        "metadata",
    );
    let tx = TxUpdateUser::new(&keypair.0, user_info.clone(), &keypair.1);
    testkit.create_block_with_transactions(txvec![tx]);
    // check result
    let schema = Schema::new(testkit.snapshot());
    let user_id_hash = schema.users_history().get(0).unwrap();
    let user_entry = schema.users().get(&user_id_hash).unwrap();
    assert_eq!(user_entry.info(), user_info);
    assert_eq!(user_entry.available_timestamps(), INITIAL_TIMESTAMPS);
    assert_eq!(user_entry.payments_hash(), &Hash::zero());
}

#[test]
fn test_modify_user_by_self() {
    let mut testkit =TestKitBuilder::validator()
        .with_service(TimestampingService::new())
        .create();
    // Execute transactions
    let keypair1 = gen_keypair();
    let user_info1 = UserInfo::new(
        "first_user",
        &keypair1.0,
        &keypair1.1[..].as_ref(),
        "metadata",
    );
    let keypair2 = gen_keypair();
    let user_info2 = UserInfo::new(
        "first_user",
        &keypair2.0,
        &keypair2.1[..].as_ref(),
        "metadata",
    );
    let tx1 = TxUpdateUser::new(
        &keypair1.0,
        user_info1.clone(),
        &keypair1.1,
    );
    let tx2 = TxUpdateUser::new(
        &keypair1.0,
        user_info2.clone(),
        &keypair1.1,
    );
    testkit.create_block_with_transactions(txvec![tx1, tx2]);
    // check result
    let schema = Schema::new(testkit.snapshot());
    let user_id_hash = schema.users_history().get(0).unwrap();
    let user_entry = schema.users().get(&user_id_hash).unwrap();
    assert_eq!(user_entry.info(), user_info2);
}

#[test]
fn test_modify_user_by_other() {
    let mut testkit =TestKitBuilder::validator()
        .with_service(TimestampingService::new())
        .create();
    // Execute transactions
    let keypair1 = gen_keypair();
    let user_info1 = UserInfo::new(
        "first_user",
        &keypair1.0,
        &keypair1.1[..].as_ref(),
        "metadata",
    );
    let keypair2 = gen_keypair();
    let user_info2 = UserInfo::new(
        "first_user",
        &keypair2.0,
        &keypair2.1[..].as_ref(),
        "metadata",
    );
    let tx1 = TxUpdateUser::new(
        &keypair1.0,
        user_info1.clone(),
        &keypair1.1,
    );
    let tx2 = TxUpdateUser::new(
        &keypair2.0,
        user_info2.clone(),
        &keypair2.1,
    );
    testkit.create_block_with_transactions(txvec![tx1, tx2]);
    // check result
    let schema = Schema::new(testkit.snapshot());
    let user_id_hash = schema.users_history().get(0).unwrap();
    let user_entry = schema.users().get(&user_id_hash).unwrap();
    assert_eq!(user_entry.info(), user_info1);
}

#[test]
fn test_add_payment_from_billing() {
    let mut testkit =TestKitBuilder::validator()
        .with_service(TimestampingService::new())
        .create();
    // Execute transactions
    let keypair = gen_keypair();
    let user_info = UserInfo::new(
        "first_user",
        &keypair.0,
        &keypair.1[..].as_ref(),
        "metadata",
    );
    let payment_info = PaymentInfo::new("first_user", 15, "metadata");
    let payment_keypair = gen_keypair();
    let tx1 = TxUpdateUser::new(&keypair.0, user_info.clone(), &keypair.1);
    let tx2 = TxPayment::new(
        &payment_keypair.0,
        payment_info,
        &payment_keypair.1,
    );
    testkit.create_block_with_transactions(txvec![tx1, tx2]);
    // check result
    let schema = Schema::new(testkit.snapshot());
    let user_id_hash = schema.users_history().get(0).unwrap();
    let user_entry = schema.users().get(&user_id_hash).unwrap();
    assert_eq!(user_entry.info(), user_info);
    assert_ne!(schema.payments("first_user").root_hash(), Hash::zero());
    assert_eq!(
        user_entry.payments_hash(),
        &schema.payments("first_user").root_hash()
    );
    assert_eq!(user_entry.available_timestamps(), 15);
}

#[test]
fn test_timestamp_by_owner_with_actual_key() {
    let mut testkit =TestKitBuilder::validator()
        .with_service(TimestampingService::new())
        .create();
    // Execute transactions
    let keypair = gen_keypair();
    let user_info = UserInfo::new(
        "first_user",
        &keypair.0,
        &keypair.1[..].as_ref(),
        "metadata",
    );
    let timestamp = Timestamp::new("first_user", &hash(&[1, 2, 4]), "metadata");
    let tx1 = TxUpdateUser::new(&keypair.0, user_info.clone(), &keypair.1);
    let tx2 = TxTimestamp::new(&keypair.0, timestamp.clone(), &keypair.1);
    testkit.create_block_with_transactions(txvec![tx1, tx2]);
    // check result
    let schema = Schema::new(testkit.snapshot());
    let user_id_hash = schema.users_history().get(0).unwrap();
    let user_entry = schema.users().get(&user_id_hash).unwrap();
    let timestamp_hash = schema.timestamps_history("first_user").get(0).unwrap();
    let timestamp_entry = schema.timestamps().get(&timestamp_hash).unwrap();
    assert_eq!(user_entry.available_timestamps(), INITIAL_TIMESTAMPS - 1);
    assert_eq!(timestamp_entry.timestamp(), timestamp);
}

#[test]
fn test_timestamp_by_other() {
    let mut testkit =TestKitBuilder::validator()
        .with_service(TimestampingService::new())
        .create();
    // Execute transactions
    let keypair = gen_keypair();
    let keypair_other = gen_keypair();
    let user_info = UserInfo::new(
        "first_user",
        &keypair.0,
        &keypair.1[..].as_ref(),
        "metadata",
    );
    let timestamp = Timestamp::new("first_user", &hash(&[1, 2, 4]), "metadata");
    let tx1 = TxUpdateUser::new(&keypair.0, user_info.clone(), &keypair.1);
    let tx2 = TxTimestamp::new(
        &keypair_other.0,
        timestamp.clone(),
        &keypair_other.1,
    );
    testkit.create_block_with_transactions(txvec![tx1, tx2]);
    // check result
    let schema = Schema::new(testkit.snapshot());
    let user_id_hash = schema.users_history().get(0).unwrap();
    let user_entry = schema.users().get(&user_id_hash).unwrap();
    // Ensure that timestamp ignored
    assert!(schema.timestamps_history("first_user").is_empty());
    assert_eq!(user_entry.available_timestamps(), INITIAL_TIMESTAMPS);
}

#[test]
fn test_timestamp_by_owner_with_old_key() {
    let mut testkit =TestKitBuilder::validator()
        .with_service(TimestampingService::new())
        .create();
    // Execute transactions
    let keypair = gen_keypair();
    let user_info = UserInfo::new(
        "first_user",
        &keypair.0,
        &keypair.1[..].as_ref(),
        "metadata",
    );
    let keypair_old = gen_keypair();
    let user_info_old = UserInfo::new(
        "first_user",
        &keypair_old.0,
        &keypair_old.1[..].as_ref(),
        "metadata",
    );
    let timestamp = Timestamp::new("first_user", &hash(&[1, 2, 4]), "metadata");
    let tx1 = TxUpdateUser::new(
        &keypair_old.0,
        user_info_old.clone(),
        &keypair_old.1,
    );
    let tx2 = TxUpdateUser::new(
        &keypair_old.0,
        user_info.clone(),
        &keypair_old.1,
    );
    let tx3 = TxTimestamp::new(
        &keypair_old.0,
        timestamp.clone(),
        &keypair_old.1,
    );
    // check result
    testkit.create_block_with_transactions(txvec![tx1, tx2, tx3]);
    let schema = Schema::new(testkit.snapshot());
    let user_id_hash = schema.users_history().get(0).unwrap();
    let user_entry = schema.users().get(&user_id_hash).unwrap();
    // Ensure that timestamp ignored
    assert!(schema.timestamps_history("first_user").is_empty());
    assert_eq!(user_entry.available_timestamps(), INITIAL_TIMESTAMPS);
}

#[test]
fn test_timestamp_without_user() {
    let mut testkit =TestKitBuilder::validator()
        .with_service(TimestampingService::new())
        .create();
    // Execute transactions
    let keypair = gen_keypair();
    let timestamp = Timestamp::new("first_user", &hash(&[1, 2, 4]), "metadata");
    let tx = TxTimestamp::new(&keypair.0, timestamp.clone(), &keypair.1);
    testkit.create_block_with_transactions(txvec![tx]);
    // check result
    let schema = Schema::new(testkit.snapshot());
    // Ensure that timestamp ignored
    assert!(schema.timestamps_history("first_user").is_empty());
}

#[test]
fn test_timestamp_exists_content_hash() {
    let mut testkit =TestKitBuilder::validator()
        .with_service(TimestampingService::new())
        .create();
    // Execute transactions
    let keypair1 = gen_keypair();
    let user_info1 = UserInfo::new(
        "first_user",
        &keypair1.0,
        &keypair1.1[..].as_ref(),
        "metadata",
    );
    let keypair2 = gen_keypair();
    let user_info2 = UserInfo::new(
        "second_user",
        &keypair2.0,
        &keypair2.1[..].as_ref(),
        "metadata",
    );
    let timestamp1 = Timestamp::new("first_user", &hash(&[1, 2, 4]), "metadata");
    let timestamp2 = Timestamp::new("second_user", &hash(&[1, 2, 4]), "metadata2");
    let tx1 = TxUpdateUser::new(
        &keypair1.0,
        user_info1.clone(),
        &keypair1.1,
    );
    let tx2 = TxUpdateUser::new(
        &keypair2.0,
        user_info2.clone(),
        &keypair2.1,
    );
    let tx3 = TxTimestamp::new(
        &keypair1.0,
        timestamp1.clone(),
        &keypair1.1,
    );
    let tx4 = TxTimestamp::new(
        &keypair2.0,
        timestamp2.clone(),
        &keypair2.1,
    );
    testkit.create_block_with_transactions(txvec![tx1, tx2, tx3, tx4]);
    // check result
    let schema = Schema::new(testkit.snapshot());
    // Make sure that timestamp2 is ignored.
    assert_eq!(schema.timestamps_history("first_user").len(), 1);
    assert!(schema.timestamps_history("second_user").is_empty());
}
