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

// cspell:ignore Ghostbusters

extern crate iron_test;

use futures::Stream;
use futures::sync::mpsc;
use serde_json;

use failure;
use iron::{IronError, Response};
use iron::headers::{ContentType, Headers};
use iron::status;
use iron::url::Url;
use self::iron_test::request::{get as test_get, post as test_post};

use api::ext::{ApiError, ApiResult, BoxedEndpoint, Endpoint, EndpointContext, EndpointSpec,
               ReadRequest, ServiceApi, TRANSACTIONS_ID};
use api::iron::{ErrorResponse, IronAdapter};
use blockchain::{Blockchain, ExecutionResult, Transaction};
use crypto::{self, CryptoHash, Hash};
use node::{ApiSender, ExternalMessage};
use storage::{Entry, Fork, MemoryDB, Snapshot};

struct Schema<T> {
    view: T,
}

impl<T: AsRef<Snapshot>> Schema<T> {
    fn new(view: T) -> Self {
        Schema { view }
    }

    fn flop(&self) -> Entry<&T, String> {
        Entry::new("flop", &self.view)
    }
}

impl<'a> Schema<&'a mut Fork> {
    fn flop_mut(&mut self) -> Entry<&mut Fork, String> {
        Entry::new("flop", self.view)
    }
}

transactions! {
    Any {
        const SERVICE_ID = 1000;

        struct Flip {
            field: u64,
        }

        struct Flop {
            field: &str
        }
    }
}

impl Transaction for Flip {
    fn verify(&self) -> bool {
        self.field() < 1_000_000_000
    }

    fn execute(&self, _: &mut Fork) -> ExecutionResult {
        Ok(())
    }
}

impl Transaction for Flop {
    fn verify(&self) -> bool {
        true
    }

    fn execute(&self, _: &mut Fork) -> ExecutionResult {
        Ok(())
    }
}

struct GetFlop;

impl EndpointSpec for GetFlop {
    type Request = ();
    type Response = Option<String>;
    const ID: &'static str = "flop";
}

impl ReadRequest for GetFlop {
    fn handle(&self, context: &EndpointContext, _: ()) -> ApiResult<Option<String>> {
        let schema = Schema::new(context.snapshot());
        Ok(schema.flop().get())
    }
}

struct GetSum;

impl EndpointSpec for GetSum {
    type Request = Vec<u32>;
    type Response = u32;
    const ID: &'static str = "sum";
}

impl ReadRequest for GetSum {
    fn handle(&self, _: &EndpointContext, numbers: Vec<u32>) -> ApiResult<u32> {
        let mut sum: u32 = 0;
        for x in numbers {
            sum = sum.checked_add(x).ok_or_else(|| {
                ApiError::InternalError("integer overflow".into())
            })?;
        }
        Ok(sum)
    }
}

fn create_blockchain() -> (Blockchain, mpsc::Receiver<ExternalMessage>) {
    use blockchain::{Service, TransactionSet};
    use encoding::Error as EncodingError;
    use messages::RawMessage;

    struct MyService;

    impl Service for MyService {
        fn service_id(&self) -> u16 {
            1000
        }

        fn service_name(&self) -> &str {
            "my-service"
        }

        fn state_hash(&self, _: &Snapshot) -> Vec<Hash> {
            vec![]
        }

        fn tx_from_raw(&self, message: RawMessage) -> Result<Box<Transaction>, EncodingError> {
            Ok(Any::tx_from_raw(message)?.into())
        }
    }

    let (pubkey, key) = crypto::gen_keypair();
    let api_channel = mpsc::channel(4);
    let blockchain = Blockchain::new(
        Box::new(MemoryDB::new()),
        vec![Box::new(MyService)],
        pubkey,
        key.clone(),
        ApiSender::new(api_channel.0.clone()),
    );

    (blockchain, api_channel.1)
}

fn create_api() -> ServiceApi {
    let mut api = ServiceApi::new();
    api.set_transactions::<Any>();
    api.insert_read(GetFlop);
    api.insert_read(sub::GetFlopOrDefault);
    api.insert_read(GetSum);
    api
}

fn assert_channel_state(receiver: &mut mpsc::Receiver<ExternalMessage>, tx_hash: &Hash) {
    let channel_message = receiver.take(1).wait().map(Result::unwrap).next().unwrap();
    assert_eq!(
        match channel_message {
            ExternalMessage::Transaction(tx) => tx.hash(),
            _ => panic!("Unexpected channel message"),
        },
        *tx_hash
    );
}

#[test]
fn test_single_transaction_sink() {
    let (_, key) = crypto::gen_keypair();
    let (blockchain, mut receiver) = create_blockchain();

    let mut api = ServiceApi::new();
    api.set_transactions::<Flip>();
    let ctx = blockchain.api_context();

    let tx = Flip::new(100, &key);
    let response = api[TRANSACTIONS_ID]
        .with_context(&ctx)
        .handle(serde_json::to_value(&tx).unwrap())
        .unwrap();
    assert_eq!(response, json!({ "tx_hash": tx.hash() }));
    assert_channel_state(receiver.by_ref(), &tx.hash());

    let tx = Flop::new("foobar", &key);
    let err = api[TRANSACTIONS_ID]
        .with_context(&ctx)
        .handle(serde_json::to_value(&tx).unwrap())
        .unwrap_err();
    match err {
        ApiError::BadRequest(e) => assert!(e.is::<serde_json::Error>()),
        _ => panic!("Incorrect type of API error"),
    }
}

#[test]
fn test_full_transaction_sink() {
    let (_, key) = crypto::gen_keypair();
    let (blockchain, mut receiver) = create_blockchain();

    let mut api = ServiceApi::new();
    api.set_transactions::<Any>();
    let ctx = blockchain.api_context();

    let tx = Flip::new(100, &key);
    let response = api[TRANSACTIONS_ID]
        .with_context(&ctx)
        .handle(serde_json::to_value(&tx).unwrap())
        .unwrap();
    assert_eq!(response, json!({ "tx_hash": tx.hash() }));
    assert_channel_state(receiver.by_ref(), &tx.hash());

    let tx = Flop::new("foobar", &key);
    let response = api[TRANSACTIONS_ID]
        .with_context(&ctx)
        .handle(serde_json::to_value(&tx).unwrap())
        .unwrap();
    assert_eq!(response, json!({ "tx_hash": tx.hash() }));
    assert_channel_state(receiver.by_ref(), &tx.hash());

    let err = api[TRANSACTIONS_ID]
        .with_context(&ctx)
        .handle(json!({ "garbage": 123 }))
        .unwrap_err();
    match err {
        ApiError::BadRequest(e) => assert!(e.is::<serde_json::Error>()),
        _ => panic!("Incorrect type of API error"),
    }
}

mod sub {
    use super::*;

    pub(super) struct GetFlopOrDefault;

    impl EndpointSpec for GetFlopOrDefault {
        type Request = String;
        type Response = String;
        const ID: &'static str = "flop-default";
    }

    impl ReadRequest for GetFlopOrDefault {
        fn handle(&self, ctx: &EndpointContext, def: String) -> ApiResult<String> {
            let schema = Schema::new(ctx.snapshot());
            Ok(schema.flop().get().unwrap_or(def))
        }
    }
}

#[test]
fn test_read_requests() {
    use self::sub::GetFlopOrDefault;

    let (mut blockchain, _) = create_blockchain();
    let api = create_api();
    let ctx = blockchain.api_context();

    let response = api[GetFlop::ID]
        .with_context(&ctx)
        .handle(json!(null))
        .unwrap();
    assert_eq!(response, json!(null));
    let response = api[GetFlopOrDefault::ID]
        .with_context(&ctx)
        .handle(json!("Ghostbusters (2016)"))
        .unwrap();
    assert_eq!(response, json!("Ghostbusters (2016)"));

    let mut fork = blockchain.fork();
    Schema::new(&mut fork).flop_mut().set(
        "The Happening".to_string(),
    );
    blockchain.merge(fork.into_patch()).unwrap();

    let response = api[GetFlop::ID]
        .with_context(&ctx)
        .handle(json!(null))
        .unwrap();
    assert_eq!(response, json!("The Happening"));
    let response = api[GetFlopOrDefault::ID]
        .with_context(&ctx)
        .handle(json!("Ghostbusters (2016)"))
        .unwrap();
    assert_eq!(response, json!("The Happening"));
}

#[test]
fn test_custom_transaction_sign_and_send() {
    use messages::Message;

    struct SendTransaction;

    impl EndpointSpec for SendTransaction {
        type Request = (u64, String);
        type Response = Hash;
        const ID: &'static str = "send-transaction";
    }

    impl Endpoint for SendTransaction {
        fn handle(&self, ctx: &mut EndpointContext, req: (u64, String)) -> Result<Hash, ApiError> {
            let tx = Flip::new_with_signature(req.0, &crypto::Signature::zero());
            let tx_hash = tx.hash();
            ctx.sign_and_send(tx)?;
            Ok(tx_hash)
        }
    }

    let (blockchain, mut receiver) = create_blockchain();
    let mut api = ServiceApi::new();
    api.insert_endpoint(SendTransaction);
    let ctx = blockchain.api_context();

    let secret_key = blockchain.api_context().secret_key().clone();
    let tx = Flip::new(500, &secret_key);
    api[SendTransaction::ID]
        .with_context(&ctx)
        .handle(json!([tx.field(), "Garbage"]))
        .unwrap();
    assert!(tx.verify_signature(blockchain.api_context().public_key()));
    assert_channel_state(receiver.by_ref(), &tx.hash());
}

#[test]
fn test_custom_transaction_send() {
    let (_, key) = crypto::gen_keypair();
    let key_clone = key.clone();
    let mut api = ServiceApi::new();
    api.insert(BoxedEndpoint::endpoint_fn("send", move |context, data| {
        let tx = Flip::new(data, &key_clone);
        let tx_hash = tx.hash();
        context.send(tx)?;
        Ok(tx_hash)
    }));

    let (blockchain, mut receiver) = create_blockchain();
    let ctx = blockchain.api_context();

    let tx = Flip::new(500, &key);
    let response = api["send"]
        .with_context(&ctx)
        .handle(json!(tx.field()))
        .unwrap();
    assert_eq!(response, json!(tx.hash()));
    assert_channel_state(receiver.by_ref(), &tx.hash());
}

#[test]
#[should_panic(expected = "Duplicate endpoint ID")]
fn test_duplicate_ids() {
    let mut api = ServiceApi::new();
    api.set_transactions::<Any>();
    api.insert(BoxedEndpoint::read_request_fn(
        TRANSACTIONS_ID,
        |_, _: ()| Ok("Gotcha!".to_owned()),
    ));
    drop(api);
}

#[test]
#[should_panic(expected = "Unknown endpoint ID")]
fn test_unknown_id() {
    let (blockchain, _) = create_blockchain();
    let api = create_api();
    api["foobar"]
        .with_context(&blockchain.api_context())
        .handle(json!(null))
        .unwrap();
}

// // // Iron-related tests // // //

fn create_url(endpoint_id: &str, q: &str) -> String {
    let mut url = Url::parse(&format!("http://localhost:3000/{}", endpoint_id)).unwrap();
    url.query_pairs_mut().append_pair("q", q);
    url.into_string()
}

fn json_from_response(resp: Response) -> serde_json::Value {
    let resp = iron_test::response::extract_body_to_string(resp);
    serde_json::from_str(&resp).unwrap()
}

fn post_headers() -> Headers {
    let mut headers = Headers::new();
    headers.set(ContentType::json());
    headers
}

#[test]
fn test_iron_read_requests_normal() {
    let (blockchain, _) = create_blockchain();
    let api = create_api();
    let handler = IronAdapter::new(blockchain.api_context()).create_handler(api);

    let resp = test_get("http://localhost:3000/flop", Headers::new(), &handler).unwrap();
    assert_eq!(resp.status, Some(status::Ok));
    assert_eq!(*resp.headers.get::<ContentType>().unwrap(), ContentType::json());
    assert_eq!(json_from_response(resp), json!(null));

    let url = create_url("flop-default", "\"The Happening\"");
    assert_eq!(
        &url,
        "http://localhost:3000/flop-default?q=%22The+Happening%22"
    );
    let resp = test_get(&url, Headers::new(), &handler).unwrap();
    assert_eq!(json_from_response(resp), json!("The Happening"));

    let url = create_url("sum", "[1, 2, 3, 4]");
    let resp = test_get(&url, Headers::new(), &handler).unwrap();
    assert_eq!(json_from_response(resp), json!(10));

    // Try read requests with POST

    let resp = test_post(
        "http://localhost:3000/sum",
        post_headers(),
        r"[
            1, 2, 3, 4,
            5, 6
        ]",
        &handler,
    ).unwrap();
    assert_eq!(json_from_response(resp), json!(21));
}

#[test]
fn test_iron_transactions_normal() {
    let (blockchain, mut receiver) = create_blockchain();
    let api = create_api();
    let handler = IronAdapter::new(blockchain.api_context()).create_handler(api);

    let tx = {
        let (_, key) = crypto::gen_keypair();
        Flip::new(200, &key)
    };

    let resp = test_post(
        "http://localhost:3000/transactions",
        post_headers(),
        &serde_json::to_string(&tx).unwrap(),
        &handler,
    ).unwrap();
    assert_eq!(json_from_response(resp), json!({ "tx_hash": tx.hash() }));
    assert_channel_state(receiver.by_ref(), &tx.hash());
}

// Checks that transactions are not processed via GET requests.
#[test]
fn test_iron_transactions_no_get() {
    let (blockchain, receiver) = create_blockchain();
    let api = create_api();
    let handler = IronAdapter::new(blockchain.api_context()).create_handler(api);

    let tx = {
        let (_, key) = crypto::gen_keypair();
        Flip::new(200, &key)
    };

    let url = create_url("transactions", &serde_json::to_string(&tx).unwrap());
    let IronError { error, response } = test_get(&url, Headers::new(), &handler).unwrap_err();
    assert_eq!(response.status, Some(status::NotFound));
    let response: ErrorResponse = serde_json::from_value(json_from_response(response)).unwrap();
    assert!(response.description.contains("Unknown endpoint"));

    let error = error
        .downcast::<failure::Compat<ApiError>>()
        .unwrap()
        .into_inner();
    match error {
        ApiError::UnknownId(id) => assert_eq!(id, *"transactions"),
        _ => panic!("Unexpected API error"),
    }

    // ensure that the transaction processing is not harmed by the receiver being dropped
    // prematurely
    drop(receiver);
}

#[test]
fn test_iron_read_requests_malformed() {
    let (blockchain, _) = create_blockchain();
    let api = create_api();
    let handler = IronAdapter::new(blockchain.api_context()).create_handler(api);

    let malformed_requests = [
        "1, 2, 3", // not correct JSON
        "[1, 2", // not correct JSON
        r#"{ "foo": "bar" }"#, // not an array
        "", // not an array
        "null", // not an array
        r#"[1, 2, "!"]"#, // array with incorrect elements
        "[-1, 2, 3]", // array with incorrect elements
    ];

    for req in malformed_requests.into_iter() {
        let url = create_url("sum", req);

        let IronError { error, response } = test_get(&url, Headers::new(), &handler).unwrap_err();
        assert_eq!(response.status, Some(status::BadRequest));
        let error = error
            .downcast::<failure::Compat<ApiError>>()
            .unwrap()
            .into_inner();
        match error {
            ApiError::BadRequest(ref e) => assert!(e.is::<serde_json::Error>()),
            _ => panic!("Unexpected API error"),
        }

        let IronError { error, response } =
            test_post("http://localhost:3000/sum", post_headers(), req, &handler).unwrap_err();
        assert_eq!(response.status, Some(status::BadRequest));
        let error = error
            .downcast::<failure::Compat<ApiError>>()
            .unwrap()
            .into_inner();
        match error {
            ApiError::BadRequest(ref e) => {
                assert!(
                e.is::<serde_json::Error>() || e.description().contains("malformed"),
                "Unexpected API error: {:?}",
                e
            )
            }
            e => panic!("Unexpected API error: {:?}", e),
        }
    }

    let url = "http://localhost:3000/sum?q[0]=5&q[1]=3";
    let IronError { response, .. } = test_get(&url, Headers::new(), &handler).unwrap_err();
    assert_eq!(response.status, Some(status::BadRequest));
}

#[test]
fn test_read_request_user_generated_internal_error() {
    let (blockchain, _) = create_blockchain();
    let api = create_api();
    let ctx = blockchain.api_context();

    let error = api[GetSum::ID]
        .with_context(&ctx)
        .handle(json!([2000000000, 2000000000, 2000000000]))
        .unwrap_err();
    match error {
        ApiError::InternalError(ref e) => assert_eq!(e.description(), "integer overflow"),
        _ => panic!("Unexpected API error"),
    }

    // Now, with the Iron engine
    let handler = IronAdapter::new(ctx).create_handler(api);

    let url = create_url("sum", "[2000000000, 2000000000, 2000000000]");
    let IronError { error, response } = test_get(&url, Headers::new(), &handler).unwrap_err();
    assert_eq!(response.status, Some(status::InternalServerError));
    let error = error
        .downcast::<failure::Compat<ApiError>>()
        .unwrap()
        .into_inner();
    match error {
        ApiError::InternalError(ref e) => assert_eq!(e.description(), "integer overflow"),
        _ => panic!("Unexpected API error"),
    }
}

#[test]
fn test_iron_transaction_verification_failure() {
    let (blockchain, receiver) = create_blockchain();
    let api = create_api();
    let handler = IronAdapter::new(blockchain.api_context()).create_handler(api);

    let tx = {
        let (_, key) = crypto::gen_keypair();
        Flip::new(2_000_000_000, &key) // fails `verify()`
    };

    let IronError { error, response } = test_post(
        "http://localhost:3000/transactions",
        post_headers(),
        &serde_json::to_string(&tx).unwrap(),
        &handler,
    ).unwrap_err();
    assert_eq!(response.status, Some(status::InternalServerError));
    let error = error
        .downcast::<failure::Compat<ApiError>>()
        .unwrap()
        .into_inner();
    match error {
        ApiError::VerificationFail(..) => {}
        e => panic!("Unexpected API error: {:?}", e),
    }

    drop(receiver);
}

#[test]
fn test_iron_transaction_send_failure() {
    use std::error::Error;

    let (blockchain, receiver) = create_blockchain();
    let api = create_api();
    let handler = IronAdapter::new(blockchain.api_context()).create_handler(api);
    drop(receiver);

    let tx = {
        let (_, key) = crypto::gen_keypair();
        Flip::new(200, &key)
    };

    // A transaction cannot be sent because the receiver stream is already dropped.
    let IronError { error, response } = test_post(
        "http://localhost:3000/transactions",
        post_headers(),
        &serde_json::to_string(&tx).unwrap(),
        &handler,
    ).unwrap_err();
    assert_eq!(response.status, Some(status::InternalServerError));
    let error = error
        .downcast::<failure::Compat<ApiError>>()
        .unwrap()
        .into_inner();
    match error {
        ApiError::TransactionNotSent(ref e) => {
            assert!(e.description().contains("receiver is gone"), "{:?}", e)
        }
        _ => panic!("Unexpected API error"),
    }
}

#[test]
fn test_not_found_error() {
    struct GetFlopOrFail;

    impl EndpointSpec for GetFlopOrFail {
        type Request = ();
        type Response = String;
        const ID: &'static str = "flop-or-fail";
    }

    impl ReadRequest for GetFlopOrFail {
        fn handle(&self, ctx: &EndpointContext, _: ()) -> Result<String, ApiError> {
            let schema = Schema::new(ctx.snapshot());
            schema.flop().get().ok_or(ApiError::NotFound)
        }
    }

    let (mut blockchain, _) = create_blockchain();
    let mut api = ServiceApi::new();
    api.insert_read(GetFlopOrFail);
    let ctx = blockchain.api_context();

    // Initially, the entry is not set, so we should get an error.
    let error = api[GetFlopOrFail::ID]
        .with_context(&ctx)
        .handle(json!(null))
        .unwrap_err();
    match error {
        ApiError::NotFound => {}
        _ => panic!("Unexpected API error"),
    }

    let handler = IronAdapter::new(blockchain.api_context()).create_handler(api);
    let IronError { response, .. } = test_get(
        "http://localhost:3000/flop-or-fail",
        Headers::new(),
        &handler,
    ).unwrap_err();
    assert_eq!(response.status, Some(status::NotFound));

    // Set the entry.
    let mut fork = blockchain.fork();
    Schema::new(&mut fork).flop_mut().set(
        "The Happening".to_string(),
    );
    blockchain.merge(fork.into_patch()).unwrap();

    let response = test_get(
        "http://localhost:3000/flop-or-fail",
        Headers::new(),
        &handler,
    ).unwrap();
    assert_eq!(json_from_response(response), json!("The Happening"));
}
