use serde_json;

use exonum::crypto::{gen_keypair, hash, Hash};
use exonum::messages::Message;

use exonum_testkit::{TestKitBuilder, ApiKind};

use TimestampingService;
use blockchain::dto::{TxUpdateUser, TxPayment, TxTimestamp, UserInfo, UserInfoEntry, PaymentInfo,
                      Timestamp, TimestampEntry};
use blockchain::schema::INITIAL_TIMESTAMPS;
use api::ItemsTemplate;

#[test]
fn test_api_get_user() {
    let mut testkit = TestKitBuilder::validator()
        .with_service(TimestampingService::new())
        .create();

    let user_info = {
        let (p, s) = gen_keypair();
        UserInfo::new("first_user", &p, &s[..].as_ref(), "metadata")
    };
    let keypair = gen_keypair();
    let tx = TxUpdateUser::new(&keypair.0, user_info.clone(), &keypair.1);
    testkit.create_block_with_transactions(txvec![tx]);

    // Checks results
    let api = testkit.api();
    let entry: UserInfoEntry = api.get(ApiKind::Service("timestamping"), "/v1/users/first_user");

    assert_eq!(entry.info(), user_info);
    assert_eq!(entry.available_timestamps(), INITIAL_TIMESTAMPS);
    assert_eq!(entry.payments_hash(), &Hash::zero());
}

#[test]
fn test_api_get_timestamp_proof() {
    let mut testkit = TestKitBuilder::validator()
        .with_service(TimestampingService::new())
        .create();

    let keypair = gen_keypair();
    // Create user
    let user_info = UserInfo::new(
        "first_user",
        &keypair.0,
        &keypair.1[..].as_ref(),
        "metadata",
    );
    let tx = TxUpdateUser::new(&keypair.0, user_info.clone(), &keypair.1);
    testkit.create_block_with_transactions(txvec![tx]);
    // Create timestamp
    let info = Timestamp::new("first_user", &Hash::zero(), "metadata");
    let tx = TxTimestamp::new(&keypair.0, info, &keypair.1);
    testkit.create_block_with_transactions(txvec![tx]);

    // get proof
    let api = testkit.api();
    let _: serde_json::Value = api.get(
        ApiKind::Service("timestamping"),
        &format!("/v1/timestamps/proof/{}", Hash::zero().to_hex()),
    );

    // TODO implement proof validation
}

#[test]
fn test_api_get_timestamp_entry() {
    let mut testkit = TestKitBuilder::validator()
        .with_service(TimestampingService::new())
        .create();

    let keypair = gen_keypair();
    // Create user
    let user_info = UserInfo::new(
        "first_user",
        &keypair.0,
        &keypair.1[..].as_ref(),
        "metadata",
    );
    let tx = TxUpdateUser::new(&keypair.0, user_info.clone(), &keypair.1);
    testkit.create_block_with_transactions(txvec![tx]);
    // Create timestamp
    let info = Timestamp::new("first_user", &Hash::zero(), "metadata");
    let tx = TxTimestamp::new(&keypair.0, info.clone(), &keypair.1);
    testkit.create_block_with_transactions(txvec![tx.clone()]);

    let api = testkit.api();
    let entry: Option<TimestampEntry> =
        api.get(
            ApiKind::Service("timestamping"),
            &format!("/v1/timestamps/value/{}", Hash::zero().to_hex()),
        );

    let entry = entry.unwrap();
    assert_eq!(entry.timestamp(), info);
    assert_eq!(entry.tx_hash(), &tx.hash());
}

#[test]
fn test_api_get_timestamps_range() {
    let mut testkit = TestKitBuilder::validator()
        .with_service(TimestampingService::new())
        .create();

    let keypair = gen_keypair();
    // Create user
    let user_info = UserInfo::new(
        "first_user",
        &keypair.0,
        &keypair.1[..].as_ref(),
        "metadata",
    );
    let tx = TxUpdateUser::new(&keypair.0, user_info.clone(), &keypair.1);
    testkit.create_block_with_transactions(txvec![tx]);
    // Create 5 timestamps
    for i in 0..5 {
        let hash = hash(&[i]);
        let info = Timestamp::new("first_user", &hash, &i.to_string());
        let tx = TxTimestamp::new(&keypair.0, info, &keypair.1);
        testkit.create_block_with_transactions(txvec![tx]);
    }
    // Api checks
    let api = testkit.api();
    // Get timestamps list
    let timestamps: ItemsTemplate<TimestampEntry> = api.get(
        ApiKind::Service("timestamping"),
        "/v1/timestamps/first_user?count=10",
    );
    assert_eq!(timestamps.items.len(), 5);
    // Get latest timestamp
    let timestamps: ItemsTemplate<TimestampEntry> = api.get(
        ApiKind::Service("timestamping"),
        "/v1/timestamps/first_user?count=1",
    );
    assert_eq!(timestamps.items.len(), 1);
    // Get first timestamp
    let timestamps: ItemsTemplate<TimestampEntry> = api.get(
        ApiKind::Service("timestamping"),
        "/v1/timestamps/first_user?count=1&from=1",
    );
    assert_eq!(timestamps.items.len(), 1);
    assert_eq!(timestamps.total_count, 5);
}

#[test]
fn test_api_get_payments_range() {
    let mut testkit = TestKitBuilder::validator()
        .with_service(TimestampingService::new())
        .create();

    let keypair = gen_keypair();
    // Create user
    let user_info = UserInfo::new(
        "first_user",
        &keypair.0,
        &keypair.1[..].as_ref(),
        "metadata",
    );
    let keypair = gen_keypair();
    let tx = TxUpdateUser::new(&keypair.0, user_info.clone(), &keypair.1);
    testkit.create_block_with_transactions(txvec![tx]);
    // Create 5 payments
    for i in 0..5 {
        let info = PaymentInfo::new("first_user", i, &i.to_string());
        let keypair = gen_keypair();
        let tx = TxPayment::new(&keypair.0, info, &keypair.1);
        testkit.create_block_with_transactions(txvec![tx]);
    }
    // Api checks
    let api = testkit.api();
    // Get payments list
    let payments: ItemsTemplate<PaymentInfo> = api.get(
        ApiKind::Service("timestamping"),
        "/v1/payments/first_user?count=10",
    );
    assert_eq!(payments.items.len(), 5);
    // Get latest payment
    let payments: ItemsTemplate<PaymentInfo> = api.get(
        ApiKind::Service("timestamping"),
        "/v1/payments/first_user?count=1",
    );
    assert_eq!(payments.items.len(), 1);
    // Get first payment
    let payments: ItemsTemplate<PaymentInfo> = api.get(
        ApiKind::Service("timestamping"),
        "/v1/payments/first_user?count=1&from=1",
    );
    assert_eq!(payments.items.len(), 1);
    assert_eq!(payments.total_count, 5);
}

#[test]
fn test_api_get_users_range() {
    let mut testkit = TestKitBuilder::validator()
        .with_service(TimestampingService::new())
        .create();

    // Create 5 users
    for i in 0..5 {
        let keypair = gen_keypair();
        // Create user
        let user_info = UserInfo::new(
            &format!("user_{}", i),
            &keypair.0,
            &keypair.1[..].as_ref(),
            &i.to_string(),
        );
        let keypair = gen_keypair();
        let tx = TxUpdateUser::new(&keypair.0, user_info.clone(), &keypair.1);
        testkit.create_block_with_transactions(txvec![tx]);
    }
    // Api checks
    let api = testkit.api();
    // Get users list
    let users: ItemsTemplate<UserInfoEntry> =
        api.get(ApiKind::Service("timestamping"), "/v1/users?count=10");
    assert_eq!(users.items.len(), 5);
    // Get latest user
    let users: ItemsTemplate<UserInfoEntry> =
        api.get(ApiKind::Service("timestamping"), "/v1/users?count=1");
    assert_eq!(users.items.len(), 1);
    // Get first user
    let users: ItemsTemplate<UserInfoEntry> =
        api.get(ApiKind::Service("timestamping"), "/v1/users?count=1&from=1");
    assert_eq!(users.items.len(), 1);
    assert_eq!(users.total_count, 5);
}
