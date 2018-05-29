// Copyright 2017 The Exonum Team
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

use bodyparser;
use exonum::api::ApiError;
use exonum::crypto;
use exonum::explorer::BlockchainExplorer;
use iron::headers::ContentType;
use iron::modifiers::Header;
use iron::prelude::*;
use iron::status::Status;
use router::Router;
use serde::Serialize;
use serde_json;

use std::num::ParseIntError;
use std::sync::{Arc, RwLock};

use super::{TestKit, TestNetworkConfiguration};

///  Creates an Iron handler for processing testkit-specific HTTP requests.
pub fn create_testkit_handler(testkit: &Arc<RwLock<TestKit>>) -> Router {
    let mut router = Router::new();

    let clone = Arc::clone(testkit);
    router.get(
        "v1/status",
        move |req: &mut Request| {
            clone
                .read()
                .expect("Cannot acquire ref to testkit")
                .handle_status(req)
        },
        "status",
    );

    let clone = Arc::clone(testkit);
    router.post(
        "v1/blocks",
        move |req: &mut Request| {
            clone
                .write()
                .expect("Cannot acquire mutable ref to testkit")
                .handle_create_block(req)
        },
        "create_block",
    );

    let clone = Arc::clone(testkit);
    router.delete(
        "v1/blocks/:height",
        move |req: &mut Request| {
            clone
                .write()
                .expect("Cannot acquire mutable ref to testkit")
                .handle_rollback(req)
        },
        "rollback",
    );

    router
}

fn ok_response<S: Serialize>(response: &S) -> IronResult<Response> {
    Ok(Response::with((
        serde_json::to_string(response).unwrap(),
        Status::Ok,
        Header(ContentType::json()),
    )))
}

trait TestKitHandler {
    fn handle_status(&self, req: &mut Request) -> IronResult<Response>;
    fn handle_create_block(&mut self, req: &mut Request) -> IronResult<Response>;
    fn handle_rollback(&mut self, req: &mut Request) -> IronResult<Response>;
}

impl TestKitHandler for TestKit {
    fn handle_status(&self, _: &mut Request) -> IronResult<Response> {
        use exonum::helpers::Height;

        #[derive(Debug, Serialize, Deserialize)]
        struct TestKitStatus {
            height: Height,
            configuration: TestNetworkConfiguration,
            next_configuration: Option<TestNetworkConfiguration>,
        }

        let status = TestKitStatus {
            height: self.height(),
            configuration: self.configuration_change_proposal(),
            next_configuration: self.next_configuration().cloned(),
        };
        ok_response(&status)
    }

    fn handle_create_block(&mut self, req: &mut Request) -> IronResult<Response> {
        #[derive(Clone, Debug, Serialize, Deserialize)]
        struct CreateBlockRequest {
            tx_hashes: Option<Vec<crypto::Hash>>,
        }

        let req = match req.get::<bodyparser::Struct<CreateBlockRequest>>() {
            Ok(Some(req)) => req,
            Ok(None) => CreateBlockRequest { tx_hashes: None },
            Err(e) => Err(ApiError::BadRequest(e.to_string()))?,
        };

        let block_info = if let Some(tx_hashes) = req.tx_hashes {
            let maybe_missing_tx = tx_hashes.iter().find(|h| !self.is_tx_in_pool(h));
            if let Some(missing_tx) = maybe_missing_tx {
                Err(ApiError::BadRequest(format!(
                    "Transaction not in mempool: {}",
                    missing_tx.to_string()
                )))?;
            }

            // NB: checkpoints must correspond 1-to-1 to blocks.
            self.checkpoint();
            self.create_block_with_tx_hashes(&tx_hashes)
        } else {
            self.checkpoint();
            self.create_block()
        };

        ok_response(&block_info)
    }

    fn handle_rollback(&mut self, req: &mut Request) -> IronResult<Response> {
        let params = req.extensions.get::<Router>().unwrap();

        let height: u64 = match params.find("height") {
            Some(height_str) => height_str
                .parse()
                .map_err(|e: ParseIntError| ApiError::BadRequest(e.to_string()))?,
            None => Err(ApiError::BadRequest(
                "Required request parameter is missing: height".to_string(),
            ))?,
        };
        if height == 0 {
            Err(ApiError::BadRequest(
                "Cannot rollback past genesis block".into(),
            ))?;
        }

        if self.height().0 >= height {
            let rollback_blocks = (self.height().0 - height + 1) as usize;
            for _ in 0..rollback_blocks {
                self.rollback();
            }
        }

        let explorer = BlockchainExplorer::new(&self.blockchain);
        let block_info = explorer.block_with_txs(self.height()).unwrap();
        ok_response(&block_info)
    }
}

#[cfg(test)]
mod tests {
    use exonum::blockchain::{ExecutionResult, Service, Transaction};
    use exonum::crypto::{CryptoHash, Hash, PublicKey};
    use exonum::encoding::{Error as EncodingError, serialize::json::ExonumJson};
    use exonum::explorer::BlockWithTransactions;
    use exonum::helpers::Height;
    use exonum::messages::{Message, RawTransaction};
    use exonum::storage::{Fork, Snapshot};
    use iron::Handler;
    use iron::headers::{ContentType, Headers};
    use iron_test::{request, response};

    use super::*;
    use TestKitBuilder;

    type DeBlock = BlockWithTransactions<serde_json::Value>;

    transactions! {
        Any {
            const SERVICE_ID = 1000;

            struct TxTimestamp {
                from: &PublicKey,
                msg: &str,
            }
        }
    }

    impl TxTimestamp {
        fn for_str(s: &str) -> Self {
            let (pubkey, key) = crypto::gen_keypair();
            TxTimestamp::new(&pubkey, s, &key)
        }
    }

    impl Transaction for TxTimestamp {
        fn verify(&self) -> bool {
            self.verify_signature(self.from())
        }

        fn execute(&self, _: &mut Fork) -> ExecutionResult {
            Ok(())
        }
    }

    /// Initializes testkit, passes it into a handler, and creates the specified number
    /// of empty blocks in the testkit blockchain.
    fn init_handler(height: Height) -> (Arc<RwLock<TestKit>>, Router) {
        struct SampleService;

        impl Service for SampleService {
            fn service_id(&self) -> u16 {
                1000
            }

            fn service_name(&self) -> &'static str {
                "sample"
            }

            fn state_hash(&self, _: &Snapshot) -> Vec<Hash> {
                Vec::new()
            }

            fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, EncodingError> {
                use exonum::blockchain::TransactionSet;

                Any::tx_from_raw(raw).map(Any::into)
            }
        }

        let testkit = TestKitBuilder::validator()
            .with_service(SampleService)
            .create();
        let testkit = Arc::new(RwLock::new(testkit));
        let (testkit, handler) = (Arc::clone(&testkit), create_testkit_handler(&testkit));

        {
            let mut testkit = testkit.write().unwrap();
            testkit.create_blocks_until(height);
        }

        (testkit, handler)
    }

    fn extract_block_info(resp: Response) -> DeBlock {
        serde_json::from_str(&response::extract_body_to_string(resp)).unwrap()
    }

    fn post_json<H, S>(url: &str, json: &S, handler: &H) -> IronResult<Response>
    where
        H: Handler,
        S: Serialize,
    {
        request::post(
            url,
            {
                let mut headers = Headers::new();
                headers.set(ContentType::json());
                headers
            },
            &serde_json::to_string(&json).unwrap(),
            handler,
        )
    }

    #[test]
    fn test_create_block_with_empty_body() {
        let (testkit, handler) = init_handler(Height(0));

        let tx = TxTimestamp::for_str("foo");
        {
            let mut testkit = testkit.write().unwrap();
            testkit.api().send(tx.clone());
            testkit.poll_events();
        }

        // Test a bodiless request
        let block_info = extract_block_info(
            request::post(
                "http://localhost:3000/v1/blocks",
                Headers::new(),
                "",
                &handler,
            ).unwrap(),
        );
        assert_eq!(block_info.header.height(), Height(1));
        assert_eq!(block_info.transactions.len(), 1);
        assert_eq!(
            *block_info.transactions[0].content(),
            tx.serialize_field().unwrap()
        );

        // Requests with a body that invoke `create_block`
        let bodies = vec![json!({}), json!({ "tx_hashes": null })];

        for body in bodies {
            {
                let mut testkit = testkit.write().unwrap();
                testkit.rollback();
                assert_eq!(testkit.height(), Height(0));
                testkit.api().send(tx.clone());
                testkit.poll_events();
            }

            let block_info = extract_block_info(
                post_json("http://localhost:3000/v1/blocks", &body, &handler).unwrap(),
            );
            assert_eq!(block_info.header.height(), Height(1));
            assert_eq!(block_info.transactions.len(), 1);
            assert_eq!(
                *block_info.transactions[0].content(),
                tx.serialize_field().unwrap()
            );
        }
    }

    #[test]
    fn test_create_block_with_specified_transactions() {
        let (testkit, handler) = init_handler(Height(0));

        let tx_foo = TxTimestamp::for_str("foo");
        let tx_bar = TxTimestamp::for_str("bar");
        {
            let mut testkit = testkit.write().unwrap();
            testkit.api().send(tx_foo.clone());
            testkit.api().send(tx_bar.clone());
            testkit.poll_events();
        }

        let body = json!({ "tx_hashes": [ tx_foo.hash().to_string() ] });
        let block_info = extract_block_info(
            post_json("http://localhost:3000/v1/blocks", &body, &handler).unwrap(),
        );
        assert_eq!(block_info.header.height(), Height(1));
        assert_eq!(block_info.transactions.len(), 1);
        assert_eq!(
            *block_info.transactions[0].content(),
            tx_foo.serialize_field().unwrap()
        );

        let body = json!({ "tx_hashes": [ tx_bar.hash().to_string() ] });
        let block_info = extract_block_info(
            post_json("http://localhost:3000/v1/blocks", &body, &handler).unwrap(),
        );
        assert_eq!(block_info.header.height(), Height(2));
        assert_eq!(block_info.transactions.len(), 1);
        assert_eq!(
            *block_info.transactions[0].content(),
            tx_bar.serialize_field().unwrap()
        );
    }

    #[test]
    fn test_create_block_with_bogus_transaction() {
        let (_, handler) = init_handler(Height(0));
        let body = json!({ "tx_hashes": [ Hash::default().to_string() ] });
        let err = post_json("http://localhost:3000/v1/blocks", &body, &handler).unwrap_err();
        assert!(
            response::extract_body_to_string(err.response).contains("Transaction not in mempool")
        );
    }

    #[test]
    fn test_rollback_normal() {
        let (testkit, handler) = init_handler(Height(0));
        for _ in 0..4 {
            post_json("http://localhost:3000/v1/blocks", &json!({}), &handler).unwrap();
        }
        assert_eq!(testkit.read().unwrap().height(), Height(4));

        // Test that requests with "overflowing" heights do nothing
        let block_info = extract_block_info(
            request::delete(
                "http://localhost:3000/v1/blocks/10",
                Headers::new(),
                &handler,
            ).unwrap(),
        );
        assert_eq!(block_info.header.height(), Height(4));

        // Test idempotence of the rollback endpoint
        for _ in 0..2 {
            let block_info = extract_block_info(
                request::delete(
                    "http://localhost:3000/v1/blocks/4",
                    Headers::new(),
                    &handler,
                ).unwrap(),
            );
            assert_eq!(block_info.header.height(), Height(3));
            {
                let testkit = testkit.read().unwrap();
                assert_eq!(testkit.height(), Height(3));
            }
        }

        // Test roll-back to the genesis block
        request::delete(
            "http://localhost:3000/v1/blocks/1",
            Headers::new(),
            &handler,
        ).unwrap();
        {
            let testkit = testkit.read().unwrap();
            assert_eq!(testkit.height(), Height(0));
        }
    }

    #[test]
    fn test_rollback_past_genesis() {
        let (_, handler) = init_handler(Height(4));

        let err = request::delete(
            "http://localhost:3000/v1/blocks/0",
            Headers::new(),
            &handler,
        ).unwrap_err();
        assert!(
            response::extract_body_to_string(err.response)
                .contains("Cannot rollback past genesis block")
        );
    }
}
