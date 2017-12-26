use std::ops::{Deref, DerefMut};
use std::cell::{Ref, RefCell};
use std::sync::{Arc, Mutex};
use std::io::{ErrorKind, Error as EventsError};

use iron::Headers;
use iron::prelude::Response;
use iron::headers::ContentType;
use iron_test;
use router::Router;
use mime::Mime;
use serde::{Serialize, Deserialize};
use serde_json;

use exonum::api::Api;
use exonum::blockchain::Transaction;
use exonum::crypto::{gen_keypair, hash, Hash, PublicKey, SecretKey};
use exonum::messages::{Message, RawMessage};
use exonum::node::TransactionSend;
use exonum::helpers;

use sandbox::sandbox::{Sandbox, sandbox_with_services};
use sandbox::sandbox_tests_helper::add_one_height_with_transactions;
use sandbox::sandbox_tests_helper::SandboxState;

use TimestampingService;
use blockchain::dto::{TxUpdateUser, TxPayment, TxTimestamp, UserInfo, UserInfoEntry, PaymentInfo,
                      Timestamp, TimestampEntry};
use blockchain::schema::INITIAL_TIMESTAMPS;
use api::{PublicApi, ItemsTemplate};

pub struct TimestampingSandbox {
    inner: Sandbox,
    state: RefCell<SandboxState>,
}

impl Deref for TimestampingSandbox {
    type Target = Sandbox;

    fn deref(&self) -> &Sandbox {
        &self.inner
    }
}

impl DerefMut for TimestampingSandbox {
    fn deref_mut(&mut self) -> &mut Sandbox {
        &mut self.inner
    }
}

impl Default for TimestampingSandbox {
    fn default() -> TimestampingSandbox {
        TimestampingSandbox::new()
    }
}

impl TimestampingSandbox {
    pub fn new() -> TimestampingSandbox {
        let sandbox = sandbox_with_services(vec![Box::new(TimestampingService::new())]);

        info!("Sandbox tests inited");

        TimestampingSandbox {
            inner: sandbox,
            state: SandboxState::new().into(),
        }
    }

    pub fn service_keypair(&self) -> (PublicKey, SecretKey) {
        gen_keypair()
    }

    pub fn state_ref(&self) -> Ref<SandboxState> {
        self.state.borrow()
    }

    pub fn add_height_with_tx<T: Message>(&self, tx: T) {
        add_one_height_with_transactions(&self.inner, &self.state_ref(), &[tx.raw().clone()]);
    }
}

pub struct TimestampingApiSandbox {
    pub router: Router,
    pub channel: TestTxSender,
}

#[derive(Debug, Default, Clone)]
pub struct TestTxSender {
    transactions: Arc<Mutex<Vec<RawMessage>>>,
}

impl TransactionSend for TestTxSender {
    fn send(&self, tx: Box<Transaction>) -> Result<(), EventsError> {
        if !tx.verify() {
            return Err(EventsError::new(ErrorKind::Other, "Unable to verify transaction"));
        }
        let rm = tx.raw().clone();
        self.transactions.lock().unwrap().push(rm);
        Ok(())
    }
}

impl TestTxSender {
    pub fn txs(&self) -> Vec<RawMessage> {
        let mut txs = self.transactions.lock().unwrap();
        let txs = txs.drain(..);
        txs.collect::<Vec<_>>()
    }
}

fn request_post<A: AsRef<str>, B, C>(router: &Router, route: A, value: B) -> C
where
    A: AsRef<str>,
    B: Serialize,
    for<'de> C: Deserialize<'de>,
{
    let body = serde_json::to_string_pretty(&serde_json::to_value(value).unwrap()).unwrap();
    let endpoint = format!("http://127.0.0.1:8000{}", route.as_ref());

    let mut headers = Headers::new();
    let mime: Mime = "application/json".parse().unwrap();
    headers.set(ContentType(mime));

    info!("POST request: `{}` body={}", endpoint, body);

    let response = iron_test::request::post(&endpoint, headers, &body, router).unwrap();
    serde_json::from_value(response_body(response)).unwrap()
}

fn request_get<A, B>(router: &Router, route: A) -> B
where
    A: AsRef<str>,
    for<'de> B: Deserialize<'de>,
{
    let endpoint = format!("http://127.0.0.1:8000{}", route.as_ref());

    info!("GET request: `{}`", endpoint);

    let response = iron_test::request::get(&endpoint, Headers::new(), router).unwrap();
    serde_json::from_value(response_body(response)).unwrap()
}

fn response_body(response: Response) -> serde_json::Value {
    if let Some(mut body) = response.body {
        let mut buf = Vec::new();
        body.write_body(&mut buf).unwrap();
        let s = String::from_utf8(buf).unwrap();
        trace!("Received response body:'{}'", &s);
        serde_json::from_str(&s).unwrap()
    } else {
        serde_json::Value::Null
    }
}

impl TimestampingApiSandbox {
    pub fn new(sandbox: &TimestampingSandbox) -> TimestampingApiSandbox {
        let mut router = Router::new();

        let channel = TestTxSender::default();
        let api = PublicApi::new(sandbox.blockchain_ref().clone(), channel.clone());
        api.wire(&mut router);

        TimestampingApiSandbox { router, channel }
    }

    pub fn post<B, C>(&self, route: &str, value: B) -> C
    where
        B: Serialize,
        for<'de> C: Deserialize<'de>,
    {
        request_post(&self.router, route, value)
    }

    pub fn get<B>(&self, route: &str) -> B
    where
        for<'de> B: Deserialize<'de>,
    {
        request_get(&self.router, route)
    }
}

#[test]
fn test_api_post_user() {
    let _ = helpers::init_logger();

    let sandbox = TimestampingSandbox::new();

    let user_info = {
        let (p, s) = gen_keypair();
        UserInfo::new("User", &p, &s[..].as_ref(), "metadata")
    };
    let keypair = sandbox.service_keypair();
    let tx = TxUpdateUser::new(&keypair.0, user_info, &keypair.1);

    let api = TimestampingApiSandbox::new(&sandbox);
    let tx_hash: Hash = api.post("/v1/users", tx.clone());
    let tx2 = TxUpdateUser::from_raw(api.channel.txs()[0].clone()).unwrap();

    assert_eq!(tx2, tx);
    assert_eq!(tx2.hash(), tx_hash);
}

#[test]
fn test_api_post_payment() {
    let _ = helpers::init_logger();

    let sandbox = TimestampingSandbox::new();

    let info = PaymentInfo::new("User", 1000, "metadata");
    let keypair = sandbox.service_keypair();
    let tx = TxPayment::new(&keypair.0, info, &keypair.1);

    let api = TimestampingApiSandbox::new(&sandbox);
    let tx_hash: Hash = api.post("/v1/payments", tx.clone());
    let tx2 = TxPayment::from_raw(api.channel.txs()[0].clone()).unwrap();

    assert_eq!(tx2, tx);
    assert_eq!(tx2.hash(), tx_hash);
}

#[test]
fn test_api_post_timestamp() {
    let _ = helpers::init_logger();

    let sandbox = TimestampingSandbox::new();

    let info = Timestamp::new("User", &Hash::zero(), "metadata");
    let keypair = gen_keypair();
    let tx = TxTimestamp::new(&keypair.0, info, &keypair.1);

    let api = TimestampingApiSandbox::new(&sandbox);
    let tx_hash: Hash = api.post("/v1/timestamps", tx.clone());
    let tx2 = TxTimestamp::from_raw(api.channel.txs()[0].clone()).unwrap();

    assert_eq!(tx2, tx);
    assert_eq!(tx2.hash(), tx_hash);
}

#[test]
fn test_api_get_user() {
    let _ = helpers::init_logger();

    let sandbox = TimestampingSandbox::new();

    let user_info = {
        let (p, s) = gen_keypair();
        UserInfo::new("first_user", &p, &s[..].as_ref(), "metadata")
    };
    let keypair = sandbox.service_keypair();
    let tx = TxUpdateUser::new(&keypair.0, user_info.clone(), &keypair.1);
    sandbox.add_height_with_tx(tx);

    // Checks results
    let api = TimestampingApiSandbox::new(&sandbox);
    let entry: UserInfoEntry = api.get("/v1/users/first_user");

    assert_eq!(entry.info(), user_info);
    assert_eq!(entry.available_timestamps(), INITIAL_TIMESTAMPS);
    assert_eq!(entry.payments_hash(), &Hash::zero());
}

#[test]
fn test_api_get_timestamp_proof() {
    let _ = helpers::init_logger();

    let sandbox = TimestampingSandbox::new();

    let keypair = gen_keypair();
    // Create user
    let user_info = UserInfo::new(
        "first_user",
        &keypair.0,
        &keypair.1[..].as_ref(),
        "metadata",
    );
    let tx = TxUpdateUser::new(&keypair.0, user_info.clone(), &keypair.1);
    sandbox.add_height_with_tx(tx);
    // Create timestamp
    let info = Timestamp::new("first_user", &Hash::zero(), "metadata");
    let tx = TxTimestamp::new(&keypair.0, info, &keypair.1);
    sandbox.add_height_with_tx(tx);

    // get proof
    let api = TimestampingApiSandbox::new(&sandbox);
    let _: serde_json::Value = api.get(&format!("/v1/timestamps/proof/{}", Hash::zero().to_hex()));

    // TODO implement proof validation
}

#[test]
fn test_api_get_timestamp_entry() {
    let _ = helpers::init_logger();

    let sandbox = TimestampingSandbox::new();

    let keypair = gen_keypair();
    // Create user
    let user_info = UserInfo::new(
        "first_user",
        &keypair.0,
        &keypair.1[..].as_ref(),
        "metadata",
    );
    let tx = TxUpdateUser::new(&keypair.0, user_info.clone(), &keypair.1);
    sandbox.add_height_with_tx(tx);
    // Create timestamp
    let info = Timestamp::new("first_user", &Hash::zero(), "metadata");
    let tx = TxTimestamp::new(&keypair.0, info.clone(), &keypair.1);
    sandbox.add_height_with_tx(tx.clone());

    let api = TimestampingApiSandbox::new(&sandbox);
    let entry: Option<TimestampEntry> =
        api.get(&format!("/v1/timestamps/value/{}", Hash::zero().to_hex()));

    let entry = entry.unwrap();
    assert_eq!(entry.timestamp(), info);
    assert_eq!(entry.tx_hash(), &tx.hash());
}

#[test]
fn test_api_get_timestamps_range() {
    let _ = helpers::init_logger();

    let sandbox = TimestampingSandbox::new();

    let keypair = gen_keypair();
    // Create user
    let user_info = UserInfo::new(
        "first_user",
        &keypair.0,
        &keypair.1[..].as_ref(),
        "metadata",
    );
    let tx = TxUpdateUser::new(&keypair.0, user_info.clone(), &keypair.1);
    sandbox.add_height_with_tx(tx);
    // Create 5 timestamps
    for i in 0..5 {
        let hash = hash(&[i]);
        let info = Timestamp::new("first_user", &hash, &i.to_string());
        let tx = TxTimestamp::new(&keypair.0, info, &keypair.1);
        sandbox.add_height_with_tx(tx);
    }
    // Api checks
    let api = TimestampingApiSandbox::new(&sandbox);
    // Get timestamps list
    let timestamps: ItemsTemplate<TimestampEntry> = api.get("/v1/timestamps/first_user?count=10");
    assert_eq!(timestamps.items.len(), 5);
    // Get latest timestamp
    let timestamps: ItemsTemplate<TimestampEntry> = api.get("/v1/timestamps/first_user?count=1");
    assert_eq!(timestamps.items.len(), 1);
    // Get first timestamp
    let timestamps: ItemsTemplate<TimestampEntry> =
        api.get("/v1/timestamps/first_user?count=1&from=1");
    assert_eq!(timestamps.items.len(), 1);
    assert_eq!(timestamps.total_count, 5);
}

#[test]
fn test_api_get_payments_range() {
    let _ = helpers::init_logger();

    let sandbox = TimestampingSandbox::new();

    let keypair = gen_keypair();
    // Create user
    let user_info = UserInfo::new(
        "first_user",
        &keypair.0,
        &keypair.1[..].as_ref(),
        "metadata",
    );
    let keypair = sandbox.service_keypair();
    let tx = TxUpdateUser::new(&keypair.0, user_info.clone(), &keypair.1);
    sandbox.add_height_with_tx(tx);
    // Create 5 payments
    for i in 0..5 {
        let info = PaymentInfo::new("first_user", i, &i.to_string());
        let keypair = sandbox.service_keypair();
        let tx = TxPayment::new(&keypair.0, info, &keypair.1);
        sandbox.add_height_with_tx(tx);
    }
    // Api checks
    let api = TimestampingApiSandbox::new(&sandbox);
    // Get payments list
    let payments: ItemsTemplate<PaymentInfo> = api.get("/v1/payments/first_user?count=10");
    assert_eq!(payments.items.len(), 5);
    // Get latest payment
    let payments: ItemsTemplate<PaymentInfo> = api.get("/v1/payments/first_user?count=1");
    assert_eq!(payments.items.len(), 1);
    // Get first payment
    let payments: ItemsTemplate<PaymentInfo> = api.get("/v1/payments/first_user?count=1&from=1");
    assert_eq!(payments.items.len(), 1);
    assert_eq!(payments.total_count, 5);
}

#[test]
fn test_api_get_users_range() {
    let _ = helpers::init_logger();

    let sandbox = TimestampingSandbox::new();

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
        let keypair = sandbox.service_keypair();
        let tx = TxUpdateUser::new(&keypair.0, user_info.clone(), &keypair.1);
        sandbox.add_height_with_tx(tx);
    }
    // Api checks
    let api = TimestampingApiSandbox::new(&sandbox);
    // Get users list
    let users: ItemsTemplate<UserInfoEntry> = api.get("/v1/users?count=10");
    assert_eq!(users.items.len(), 5);
    // Get latest user
    let users: ItemsTemplate<UserInfoEntry> = api.get("/v1/users?count=1");
    assert_eq!(users.items.len(), 1);
    // Get first user
    let users: ItemsTemplate<UserInfoEntry> = api.get("/v1/users?count=1&from=1");
    assert_eq!(users.items.len(), 1);
    assert_eq!(users.total_count, 5);
}
