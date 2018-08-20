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

use exonum::{
    api::{self, ApiAggregator, ServiceApiBuilder, ServiceApiScope, ServiceApiState},
    blockchain::{SharedNodeState, Transaction}, crypto::Hash,
    explorer::{BlockWithTransactions, BlockchainExplorer}, helpers::Height,
};

use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use super::{TestKit, TestNetworkConfiguration};

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CreateBlockQuery {
    tx_hashes: Option<Vec<Hash>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TestKitStatus {
    height: Height,
    configuration: TestNetworkConfiguration,
    next_configuration: Option<TestNetworkConfiguration>,
}

#[derive(Debug, Clone)]
struct TestkitServerApi(Arc<RwLock<TestKit>>);

impl TestkitServerApi {
    fn read(&self) -> RwLockReadGuard<TestKit> {
        self.0.read().unwrap()
    }

    fn write(&self) -> RwLockWriteGuard<TestKit> {
        self.0.write().unwrap()
    }

    fn status(&self) -> api::Result<TestKitStatus> {
        let testkit = self.read();
        Ok(TestKitStatus {
            height: testkit.height(),
            configuration: testkit.configuration_change_proposal(),
            next_configuration: testkit.next_configuration().cloned(),
        })
    }

    fn create_block(
        &self,
        tx_hashes: Option<Vec<Hash>>,
    ) -> api::Result<BlockWithTransactions<Box<dyn Transaction>>> {
        let mut testkit = self.write();
        let block_info = if let Some(tx_hashes) = tx_hashes {
            let maybe_missing_tx = tx_hashes.iter().find(|h| !testkit.is_tx_in_pool(h));
            if let Some(missing_tx) = maybe_missing_tx {
                Err(api::Error::BadRequest(format!(
                    "Transaction not in mempool: {}",
                    missing_tx.to_string()
                )))?;
            }

            // NB: checkpoints must correspond 1-to-1 to blocks.
            testkit.checkpoint();
            testkit.create_block_with_tx_hashes(&tx_hashes)
        } else {
            testkit.checkpoint();
            testkit.create_block()
        };
        Ok(block_info)
    }

    fn rollback(
        &self,
        height: Height,
    ) -> api::Result<Option<BlockWithTransactions<Box<dyn Transaction>>>> {
        if height == Height(0) {
            Err(api::Error::BadRequest(
                "Cannot rollback past genesis block".into(),
            ))?;
        }

        let mut testkit = self.write();
        if testkit.height() >= height {
            let rollback_blocks = (testkit.height().0 - height.0 + 1) as usize;
            for _ in 0..rollback_blocks {
                testkit.rollback();
            }
        }

        let explorer = BlockchainExplorer::new(&testkit.blockchain);
        Ok(explorer.block_with_txs(testkit.height()))
    }

    fn handle_status(self, name: &'static str, api_scope: &mut ServiceApiScope) -> Self {
        let self_ = self.clone();
        api_scope.endpoint(name, move |_state: &ServiceApiState, _query: ()| {
            self.status()
        });
        self_
    }

    fn handle_create_block(self, name: &'static str, api_scope: &mut ServiceApiScope) -> Self {
        let self_ = self.clone();
        api_scope.endpoint_mut(
            name,
            move |_state: &ServiceApiState, query: Option<CreateBlockQuery>| {
                self.create_block(query.and_then(|query| query.tx_hashes))
            },
        );
        self_
    }

    fn handle_rollback(self, name: &'static str, api_scope: &mut ServiceApiScope) -> Self {
        let self_ = self.clone();
        api_scope.endpoint_mut(name, move |_state: &ServiceApiState, height: Height| {
            self.rollback(height)
        });
        self_
    }

    fn wire(self, builder: &mut ServiceApiBuilder) {
        let api_scope = builder.private_scope();
        self.handle_status("v1/status", api_scope)
            .handle_rollback("v1/blocks/rollback", api_scope)
            .handle_create_block("v1/blocks/create", api_scope);
    }
}

///  Creates an API handlers for processing testkit-specific HTTP requests.
pub fn create_testkit_handlers(inner: &Arc<RwLock<TestKit>>) -> ServiceApiBuilder {
    let mut builder = ServiceApiBuilder::new();
    let server_api = TestkitServerApi(inner.clone());
    server_api.wire(&mut builder);
    builder
}

/// Creates an ApiAggregator with the testkit server specific handlers.
pub fn create_testkit_api_aggregator(testkit: &Arc<RwLock<TestKit>>) -> ApiAggregator {
    let mut aggregator = ApiAggregator::new(
        testkit.read().unwrap().blockchain().clone(),
        SharedNodeState::new(10_000),
    );
    aggregator.insert("testkit", create_testkit_handlers(testkit));
    aggregator
}

#[cfg(test)]
mod tests {
    use serde_json;

    use exonum::api;
    use exonum::blockchain::{ExecutionResult, Service, Transaction};
    use exonum::crypto::{gen_keypair, CryptoHash, Hash, PublicKey};
    use exonum::encoding::{serialize::json::ExonumJson, Error as EncodingError};
    use exonum::explorer::BlockWithTransactions;
    use exonum::helpers::Height;
    use exonum::messages::{Message, RawTransaction};
    use exonum::storage::{Fork, Snapshot};

    use super::*;
    use {TestKitApi, TestKitBuilder};

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
            let (pubkey, key) = gen_keypair();
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
    fn init_handler(height: Height) -> (Arc<RwLock<TestKit>>, TestKitApi) {
        struct SampleService;

        impl Service for SampleService {
            fn service_id(&self) -> u16 {
                1000
            }

            fn service_name(&self) -> &'static str {
                "sample"
            }

            fn state_hash(&self, _: &dyn Snapshot) -> Vec<Hash> {
                Vec::new()
            }

            fn tx_from_raw(
                &self,
                raw: RawTransaction,
            ) -> Result<Box<dyn Transaction>, EncodingError> {
                use exonum::blockchain::TransactionSet;

                Any::tx_from_raw(raw).map(Any::into)
            }
        }

        let testkit = TestKitBuilder::validator()
            .with_service(SampleService)
            .create();

        let api_sender = testkit.api_sender.clone();
        let testkit = Arc::new(RwLock::new(testkit));
        let aggregator = create_testkit_api_aggregator(&testkit);
        let (testkit, api) = (
            Arc::clone(&testkit),
            TestKitApi::from_raw_parts(aggregator, api_sender),
        );

        testkit.write().unwrap().create_blocks_until(height);
        (testkit, api)
    }

    #[test]
    fn test_create_block_with_empty_body() {
        let (testkit, api) = init_handler(Height(0));

        let tx = TxTimestamp::for_str("foo");
        {
            let mut testkit = testkit.write().unwrap();
            api.send(tx.clone());
            testkit.poll_events();
        }

        // Test a bodiless request
        let block_info: DeBlock = api.private("api/testkit")
            .query(&CreateBlockQuery { tx_hashes: None })
            .post("v1/blocks/create")
            .unwrap();

        assert_eq!(block_info.header.height(), Height(1));
        assert_eq!(block_info.transactions.len(), 1);
        assert_eq!(
            *block_info.transactions[0].content(),
            tx.serialize_field().unwrap()
        );

        // Requests with a body that invoke `create_block`
        let bodies = vec![None, Some(CreateBlockQuery { tx_hashes: None })];

        for body in &bodies {
            {
                let mut testkit = testkit.write().unwrap();
                testkit.rollback();
                assert_eq!(testkit.height(), Height(0));
                api.send(tx.clone());
                testkit.poll_events();
            }

            let block_info: DeBlock = api.private("api/testkit")
                .query(body)
                .post("v1/blocks/create")
                .unwrap();

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
        let (testkit, api) = init_handler(Height(0));

        let tx_foo = TxTimestamp::for_str("foo");
        let tx_bar = TxTimestamp::for_str("bar");
        {
            let mut testkit = testkit.write().unwrap();
            api.send(tx_foo.clone());
            api.send(tx_bar.clone());
            testkit.poll_events();
        }

        let body = CreateBlockQuery {
            tx_hashes: Some(vec![tx_foo.hash()]),
        };
        let block_info: DeBlock = api.private("api/testkit")
            .query(&body)
            .post("v1/blocks/create")
            .unwrap();

        assert_eq!(block_info.header.height(), Height(1));
        assert_eq!(block_info.transactions.len(), 1);
        assert_eq!(
            *block_info.transactions[0].content(),
            tx_foo.serialize_field().unwrap()
        );

        let body = CreateBlockQuery {
            tx_hashes: Some(vec![tx_bar.hash()]),
        };
        let block_info: DeBlock = api.private("api/testkit")
            .query(&body)
            .post("v1/blocks/create")
            .unwrap();

        assert_eq!(block_info.header.height(), Height(2));
        assert_eq!(block_info.transactions.len(), 1);
        assert_eq!(
            *block_info.transactions[0].content(),
            tx_bar.serialize_field().unwrap()
        );
    }

    #[test]
    fn test_create_block_with_bogus_transaction() {
        let (_, api) = init_handler(Height(0));

        let body = CreateBlockQuery {
            tx_hashes: Some(vec![Hash::zero()]),
        };
        let err = api.private("api/testkit")
            .query(&body)
            .post::<DeBlock>("v1/blocks/create")
            .unwrap_err();

        assert_matches!(
            err,
            api::Error::BadRequest(ref body) if body.starts_with("Transaction not in mempool")
        );
    }

    #[test]
    fn test_rollback_normal() {
        let (testkit, api) = init_handler(Height(0));

        for _ in 0..4 {
            api.private("api/testkit")
                .query(&CreateBlockQuery { tx_hashes: None })
                .post::<DeBlock>("v1/blocks/create")
                .unwrap();
        }
        assert_eq!(testkit.read().unwrap().height(), Height(4));

        // Test that requests with "overflowing" heights do nothing
        let block_info: DeBlock = api.private("api/testkit")
            .query(&Height(10))
            .post("v1/blocks/rollback")
            .unwrap();
        assert_eq!(block_info.header.height(), Height(4));

        // Test idempotence of the rollback endpoint
        for _ in 0..2 {
            let block_info: DeBlock = api.private("api/testkit")
                .query(&Height(4))
                .post("v1/blocks/rollback")
                .unwrap();

            assert_eq!(block_info.header.height(), Height(3));
            {
                let testkit = testkit.read().unwrap();
                assert_eq!(testkit.height(), Height(3));
            }
        }

        // Test roll-back to the genesis block
        api.private("api/testkit")
            .query(&Height(1))
            .post::<DeBlock>("v1/blocks/rollback")
            .unwrap();
        {
            let testkit = testkit.read().unwrap();
            assert_eq!(testkit.height(), Height(0));
        }
    }

    #[test]
    fn test_rollback_past_genesis() {
        let (_, api) = init_handler(Height(4));

        let err = api.private("api/testkit")
            .query(&Height(0))
            .post::<DeBlock>("v1/blocks/rollback")
            .unwrap_err();

        assert_matches!(
            err,
            api::Error::BadRequest(ref body) if body == "Cannot rollback past genesis block"
        );
    }
}
