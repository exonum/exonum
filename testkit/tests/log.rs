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

#[macro_use]
extern crate exonum;
extern crate exonum_testkit;
extern crate iron;
extern crate iron_test;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
#[macro_use]
extern crate pretty_assertions;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate thread_local;

use exonum::api::public::BlocksRange;
use exonum::crypto::{self, CryptoHash, Hash};
use exonum::helpers::Height;
use exonum_testkit::{ApiKind, TestKit, TestKitApi, TestKitBuilder};
use log::{set_boxed_logger, set_max_level, Level, Log, Metadata, Record};
use thread_local::ThreadLocal;

use std::cell::RefCell;
use std::sync::Arc;

use counter::{CounterService, TransactionResponse, TxIncrement, TxReset};

#[path = "counter/counter.rs"]
mod counter;

lazy_static! {
    static ref LOG: DebugLog = DebugLog::new();
}

type LogEntries = RefCell<Vec<String>>;

#[derive(Clone)]
struct DebugLog {
    messages: Arc<ThreadLocal<LogEntries>>,
}

impl DebugLog {
    fn new() -> Self {
        DebugLog {
            messages: Arc::new(ThreadLocal::new()),
        }
    }

    fn pop_message(&self) -> Option<String> {
        let messages = self.messages.get()?;
        messages.borrow_mut().pop()
    }

    fn is_empty(&self) -> bool {
        self.messages
            .get()
            .map(|cell| cell.borrow().is_empty())
            .unwrap_or(true)
    }
}

impl Log for DebugLog {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.target().starts_with("exonum_testkit::api") && metadata.level() == Level::Trace
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let cell = self.messages.get_or(|| Box::new(RefCell::new(vec![])));
        cell.borrow_mut().push(format!("{}", record.args()));
    }

    fn flush(&self) {}
}

fn init_testkit() -> (TestKit, TestKitApi) {
    let testkit = TestKitBuilder::validator()
        .with_service(CounterService)
        .create();
    let api = testkit.api();
    (testkit, api)
}

fn init_log() {
    set_boxed_logger(Box::new(LOG.clone())).ok();
    set_max_level("trace".parse().unwrap());
}

#[test]
fn test_get_api() {
    init_log();
    let (mut testkit, api) = init_testkit();
    let (pubkey, key) = crypto::gen_keypair();

    testkit.create_block_with_transaction(TxIncrement::new(&pubkey, 5, &key));

    let count: u64 = api.get(ApiKind::Service("counter"), "count");
    assert_eq!(count, 5);

    let s = LOG.pop_message().expect("no message received");
    assert!(LOG.is_empty());
    assert!(
        s.starts_with("GET (public) /api/services/counter/count\n"),
        "{}",
        s
    );
    assert!(s.contains("Response: 200 OK\n5"), "{}", s);
}

#[test]
fn test_public_builtin_api() {
    init_log();
    let (mut testkit, api) = init_testkit();
    let (pubkey, key) = crypto::gen_keypair();

    let tx = TxIncrement::new(&pubkey, 5, &key);
    testkit.create_block_with_transaction(tx.clone());

    let BlocksRange { blocks, .. } = api.get(ApiKind::Explorer, "v1/blocks?count=10");
    assert_eq!(blocks.len(), 2);
    assert_eq!(blocks[0].height(), Height(1));

    let s = LOG.pop_message().expect("no message received");
    assert!(LOG.is_empty());
    assert!(
        s.starts_with("GET (public) /api/explorer/v1/blocks?count=10\n"),
        "{}",
        s
    );
    assert!(s.contains("Response: 200 OK\n"), "{}", s);

    let _: serde_json::Value =
        api.get(ApiKind::Explorer, &format!("v1/transactions/{}", tx.hash()));

    let s = LOG.pop_message().expect("no message received");
    assert!(LOG.is_empty());
    assert!(
        s.starts_with("GET (public) /api/explorer/v1/transactions/"),
        "{}",
        s
    );
    assert!(s.contains("Response: 200 OK\n"), "{}", s);
}

#[test]
fn test_private_api() {
    init_log();
    let (mut testkit, api) = init_testkit();
    let (pubkey, key) = crypto::gen_keypair();

    testkit.create_block_with_transaction(TxIncrement::new(&pubkey, 8, &key));
    let counter: u64 = api.get_private(ApiKind::Service("counter"), "count");
    assert_eq!(counter, 8);

    let s = LOG.pop_message().expect("no message received");
    assert!(LOG.is_empty());
    assert!(
        s.starts_with("GET (private) /api/services/counter/count"),
        "{}",
        s
    );
    assert!(s.contains("Response: 200 OK\n"), "{}", s);
}

#[test]
fn test_not_found_response() {
    init_log();
    let (_, api) = init_testkit();

    api.get_err(
        ApiKind::Explorer,
        &format!("v1/transactions/{}", Hash::zero()),
    );

    let s = LOG.pop_message().expect("no message received");
    assert!(LOG.is_empty());
    assert!(
        s.starts_with("GET (public) /api/explorer/v1/transactions/"),
        "{}",
        s
    );
    assert!(s.contains("Response: 404 Not Found\n"), "{}", s);
}

#[test]
fn test_post_transaction() {
    init_log();
    let (mut testkit, api) = init_testkit();

    let (pubkey, key) = crypto::gen_keypair_from_seed(&crypto::Seed::from_slice(
        &crypto::hash(b"correct horse battery staple")[..],
    ).unwrap());

    let tx = TxReset::new(&pubkey, &key);
    let _: TransactionResponse = api.post_private(ApiKind::Service("counter"), "reset", &tx);
    testkit.poll_events();
    assert!(testkit.is_tx_in_pool(&tx.hash()));

    let s = LOG.pop_message().expect("no message received");
    assert!(LOG.is_empty());
    assert!(
        s.starts_with("POST (private) /api/services/counter/reset\nBody: {"),
        "{}",
        s
    );
    assert!(s.contains("Response: 200 OK\n"), "{}", s);
}

#[test]
fn test_custom_request() {
    use iron::headers::Headers;
    use iron_test::request;

    init_log();
    let (mut testkit, api) = init_testkit();

    request::get(
        "http://localhost:3000/api/explorer/v1/blocks/0",
        Headers::new(),
        api.public_handler(),
    ).unwrap();
    testkit.poll_events();

    let s = LOG.pop_message().expect("no message received");
    assert!(LOG.is_empty());
    assert!(
        s.starts_with("GET (public) /api/explorer/v1/blocks/0"),
        "{}",
        s
    );
}
