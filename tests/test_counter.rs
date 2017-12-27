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

#[macro_use]
extern crate exonum;
#[macro_use]
extern crate exonum_testkit;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;

use exonum::crypto::{self, PublicKey};
use exonum::helpers::Height;
use exonum::messages::Message;
use exonum::encoding::serialize::FromHex;
use exonum::encoding::serialize::json::ExonumJson;
use exonum_testkit::{ApiKind, ComparableSnapshot, TestKit, TestKitApi, TestKitBuilder};

mod counter {
    //! Sample counter service.

    extern crate bodyparser;
    extern crate iron;
    extern crate router;

    use exonum::blockchain::{ApiContext, Blockchain, Service, Transaction};
    use exonum::messages::{Message, RawTransaction};
    use exonum::node::{ApiSender, TransactionSend};
    use exonum::storage::{Entry, Fork, Snapshot};
    use exonum::crypto::{Hash, PublicKey};
    use exonum::encoding;
    use exonum::api::{Api, ApiError};
    use self::iron::Handler;
    use self::iron::prelude::*;
    use self::router::Router;
    use serde_json;

    const SERVICE_ID: u16 = 1;
    const TX_INCREMENT_ID: u16 = 1;

    // "correct horse battery staple" brainwallet pubkey in Ed25519 with SHA-256 digest
    pub const ADMIN_KEY: &str = "506f27b1b4c2403f2602d663a059b0262afd6a5bcda95a08dd96a4614a89f1b0";

    // // // // Schema // // // //

    pub struct CounterSchema<T> {
        view: T,
    }

    impl<T: AsRef<Snapshot>> CounterSchema<T> {
        pub fn new(view: T) -> Self {
            CounterSchema { view }
        }

        fn entry(&self) -> Entry<&Snapshot, u64> {
            Entry::new("counter.count", self.view.as_ref())
        }

        pub fn count(&self) -> Option<u64> {
            self.entry().get()
        }
    }

    impl<'a> CounterSchema<&'a mut Fork> {
        fn entry_mut(&mut self) -> Entry<&mut Fork, u64> {
            Entry::new("counter.count", self.view)
        }

        fn inc_count(&mut self, inc: u64) -> u64 {
            let count = self.count().unwrap_or(0) + inc;
            self.entry_mut().set(count);
            count
        }

        fn set_count(&mut self, count: u64) {
            self.entry_mut().set(count);
        }
    }

    // // // // Transactions // // // //

    message! {
        struct TxIncrement {
            const TYPE = SERVICE_ID;
            const ID = TX_INCREMENT_ID;
            const SIZE = 40;

            field author: &PublicKey [0 => 32]
            field by: u64 [32 => 40]
        }
    }

    impl Transaction for TxIncrement {
        fn verify(&self) -> bool {
            self.verify_signature(self.author())
        }

        fn execute(&self, fork: &mut Fork) {
            let mut schema = CounterSchema::new(fork);
            schema.inc_count(self.by());
        }
    }

    message! {
        struct TxReset {
            const TYPE = SERVICE_ID;
            const ID = TX_INCREMENT_ID;
            const SIZE = 32;

            field author: &PublicKey [0 => 32]
        }
    }

    impl TxReset {
        pub fn verify_author(&self) -> bool {
            use exonum::encoding::serialize::FromHex;
            *self.author() == PublicKey::from_hex(ADMIN_KEY).unwrap()
        }
    }

    impl Transaction for TxReset {
        fn verify(&self) -> bool {
            self.verify_author() && self.verify_signature(self.author())
        }

        fn execute(&self, fork: &mut Fork) {
            let mut schema = CounterSchema::new(fork);
            schema.set_count(0);
        }
    }

    // // // // API // // // //

    #[derive(Serialize, Deserialize)]
    pub struct TransactionResponse {
        pub tx_hash: Hash,
    }

    #[derive(Clone)]
    struct CounterApi {
        channel: ApiSender,
        blockchain: Blockchain,
    }

    impl CounterApi {
        fn increment(&self, req: &mut Request) -> IronResult<Response> {
            match req.get::<bodyparser::Struct<TxIncrement>>() {
                Ok(Some(transaction)) => {
                    let transaction: Box<Transaction> = Box::new(transaction);
                    let tx_hash = transaction.hash();
                    self.channel.send(transaction).map_err(ApiError::from)?;
                    let json = TransactionResponse { tx_hash };
                    self.ok_response(&serde_json::to_value(&json).unwrap())
                }
                Ok(None) => Err(ApiError::IncorrectRequest("Empty request body".into()))?,
                Err(e) => Err(ApiError::IncorrectRequest(Box::new(e)))?,
            }
        }

        fn count(&self) -> Option<u64> {
            let view = self.blockchain.snapshot();
            let schema = CounterSchema::new(&view);
            schema.count()
        }

        fn get_count(&self, _: &mut Request) -> IronResult<Response> {
            let count = self.count().unwrap_or(0);
            self.ok_response(&serde_json::to_value(count).unwrap())
        }

        fn reset(&self, req: &mut Request) -> IronResult<Response> {
            match req.get::<bodyparser::Struct<TxReset>>() {
                Ok(Some(transaction)) => {
                    let transaction: Box<Transaction> = Box::new(transaction);
                    let tx_hash = transaction.hash();
                    self.channel.send(transaction).map_err(ApiError::from)?;
                    let json = TransactionResponse { tx_hash };
                    self.ok_response(&serde_json::to_value(&json).unwrap())
                }
                Ok(None) => Err(ApiError::IncorrectRequest("Empty request body".into()))?,
                Err(e) => Err(ApiError::IncorrectRequest(Box::new(e)))?,
            }
        }

        fn wire_private(&self, router: &mut Router) {
            let self_ = self.clone();
            let reset = move |req: &mut Request| self_.reset(req);
            router.post("/reset", reset, "reset");
        }
    }

    impl Api for CounterApi {
        fn wire(&self, router: &mut Router) {
            let self_ = self.clone();
            let increment = move |req: &mut Request| self_.increment(req);
            router.post("/count", increment, "increment");

            let self_ = self.clone();
            let get_count = move |req: &mut Request| self_.get_count(req);
            router.get("/count", get_count, "get_count");
        }
    }

    // // // // Service // // // //

    pub struct CounterService;

    impl Service for CounterService {
        fn service_name(&self) -> &'static str {
            "counter"
        }

        fn state_hash(&self, _: &Snapshot) -> Vec<Hash> {
            Vec::new()
        }

        fn service_id(&self) -> u16 {
            SERVICE_ID
        }

        /// Implement a method to deserialize transactions coming to the node.
        fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, encoding::Error> {
            let trans: Box<Transaction> = match raw.message_type() {
                TX_INCREMENT_ID => Box::new(TxIncrement::from_raw(raw)?),
                _ => {
                    return Err(encoding::Error::IncorrectMessageType {
                        message_type: raw.message_type(),
                    });
                }
            };
            Ok(trans)
        }

        /// Create a REST `Handler` to process web requests to the node.
        fn public_api_handler(&self, ctx: &ApiContext) -> Option<Box<Handler>> {
            let mut router = Router::new();
            let api = CounterApi {
                channel: ctx.node_channel().clone(),
                blockchain: ctx.blockchain().clone(),
            };
            api.wire(&mut router);
            Some(Box::new(router))
        }

        fn private_api_handler(&self, ctx: &ApiContext) -> Option<Box<Handler>> {
            let mut router = Router::new();
            let api = CounterApi {
                channel: ctx.node_channel().clone(),
                blockchain: ctx.blockchain().clone(),
            };
            api.wire_private(&mut router);
            Some(Box::new(router))
        }
    }
}

use counter::{CounterSchema, CounterService, TransactionResponse, TxIncrement, TxReset, ADMIN_KEY};

fn init_testkit() -> (TestKit, TestKitApi) {
    let testkit = TestKitBuilder::validator()
        .with_service(CounterService)
        .create();
    let api = testkit.api();
    (testkit, api)
}

fn inc_count(api: &TestKitApi, by: u64) -> TxIncrement {
    let (pubkey, key) = crypto::gen_keypair();
    // Create a presigned transaction
    let tx = TxIncrement::new(&pubkey, by, &key);

    let tx_info: TransactionResponse = api.post(ApiKind::Service("counter"), "count", &tx);
    assert_eq!(tx_info.tx_hash, tx.hash());
    tx
}

#[test]
fn test_inc_count_create_block() {
    let (mut testkit, api) = init_testkit();
    let (pubkey, key) = crypto::gen_keypair();

    // Create a presigned transaction
    testkit.create_block_with_transaction(TxIncrement::new(&pubkey, 5, &key));

    // Check that the user indeed is persisted by the service
    let counter: u64 = api.get(ApiKind::Service("counter"), "count");
    assert_eq!(counter, 5);

    testkit.create_block_with_transactions(txvec![
        TxIncrement::new(&pubkey, 4, &key),
        TxIncrement::new(&pubkey, 1, &key),
    ]);

    let counter: u64 = api.get(ApiKind::Service("counter"), "count");
    assert_eq!(counter, 10);
}

#[should_panic(expected = "Transaction is already committed")]
#[test]
fn test_inc_count_create_block_with_committed_transaction() {
    let (mut testkit, _) = init_testkit();
    let (pubkey, key) = crypto::gen_keypair();
    // Create a presigned transaction
    testkit.create_block_with_transaction(TxIncrement::new(&pubkey, 5, &key));
    // Create another block with the same transaction
    testkit.create_block_with_transaction(TxIncrement::new(&pubkey, 5, &key));
}

#[test]
fn test_inc_count_api() {
    let (mut testkit, api) = init_testkit();
    inc_count(&api, 5);
    testkit.create_block();

    // Check that the user indeed is persisted by the service
    let counter: u64 = api.get(ApiKind::Service("counter"), "count");
    assert_eq!(counter, 5);
}

#[test]
fn test_inc_count_with_multiple_transactions() {
    let (mut testkit, api) = init_testkit();

    for _ in 0..100 {
        inc_count(&api, 1);
        inc_count(&api, 2);
        inc_count(&api, 3);
        inc_count(&api, 4);

        testkit.create_block();
    }

    assert_eq!(testkit.height(), Height(100));
    let counter: u64 = api.get(ApiKind::Service("counter"), "count");
    assert_eq!(counter, 1_000);
}

#[test]
fn test_inc_count_with_manual_tx_control() {
    let (mut testkit, api) = init_testkit();
    let tx_a = inc_count(&api, 5);
    let tx_b = inc_count(&api, 3);

    // Empty block
    testkit.create_block_with_tx_hashes(&[]);
    let counter: u64 = api.get(ApiKind::Service("counter"), "count");
    assert_eq!(counter, 0);

    testkit.create_block_with_tx_hashes(&[tx_b.hash()]);
    let counter: u64 = api.get(ApiKind::Service("counter"), "count");
    assert_eq!(counter, 3);

    testkit.create_block_with_tx_hashes(&[tx_a.hash()]);
    let counter: u64 = api.get(ApiKind::Service("counter"), "count");
    assert_eq!(counter, 8);
}

#[test]
fn test_private_api() {
    let (mut testkit, api) = init_testkit();
    inc_count(&api, 5);
    inc_count(&api, 3);

    testkit.create_block();
    let counter: u64 = api.get(ApiKind::Service("counter"), "count");
    assert_eq!(counter, 8);

    let (pubkey, key) = crypto::gen_keypair_from_seed(&crypto::Seed::from_slice(
        &crypto::hash(b"correct horse battery staple")[..],
    ).unwrap());
    assert_eq!(pubkey, PublicKey::from_hex(ADMIN_KEY).unwrap());

    let tx = TxReset::new(&pubkey, &key);
    let tx_info: TransactionResponse = api.post_private(ApiKind::Service("counter"), "reset", &tx);
    assert_eq!(tx_info.tx_hash, tx.hash());

    testkit.create_block();
    let counter: u64 = api.get(ApiKind::Service("counter"), "count");
    assert_eq!(counter, 0);
}

#[test]
fn test_probe() {
    let (mut testkit, api) = init_testkit();

    let tx = {
        let (pubkey, key) = crypto::gen_keypair();
        TxIncrement::new(&pubkey, 5, &key)
    };

    let snapshot = testkit.probe(tx.clone());
    let schema = CounterSchema::new(&snapshot);
    assert_eq!(schema.count(), Some(5));
    // Verify that the patch has not been applied to the blockchain
    let counter: u64 = api.get(ApiKind::Service("counter"), "count");
    assert_eq!(counter, 0);

    let other_tx = {
        let (pubkey, key) = crypto::gen_keypair();
        TxIncrement::new(&pubkey, 3, &key)
    };

    let snapshot = testkit.probe_all(txvec![tx.clone(), other_tx.clone()]);
    let schema = CounterSchema::new(&snapshot);
    assert_eq!(schema.count(), Some(8));

    // Posting a transaction is not enough to change the blockchain!
    let _: TransactionResponse = api.post(ApiKind::Service("counter"), "count", &tx);
    let snapshot = testkit.probe(other_tx.clone());
    let schema = CounterSchema::new(&snapshot);
    assert_eq!(schema.count(), Some(3));

    testkit.create_block();
    let snapshot = testkit.probe(other_tx.clone());
    let schema = CounterSchema::new(&snapshot);
    assert_eq!(schema.count(), Some(8));
}

#[test]
fn test_duplicate_tx() {
    let (mut testkit, api) = init_testkit();

    let tx = inc_count(&api, 5);
    testkit.create_block();
    let _: TransactionResponse = api.post(ApiKind::Service("counter"), "count", &tx);
    let _: TransactionResponse = api.post(ApiKind::Service("counter"), "count", &tx);
    testkit.create_block();
    let counter: u64 = api.get(ApiKind::Service("counter"), "count");
    assert_eq!(counter, 5);
}

#[test]
fn test_probe_advanced() {
    let (mut testkit, api) = init_testkit();

    let tx = {
        let (pubkey, key) = crypto::gen_keypair();
        TxIncrement::new(&pubkey, 6, &key)
    };
    let other_tx = {
        let (pubkey, key) = crypto::gen_keypair();
        TxIncrement::new(&pubkey, 10, &key)
    };
    let admin_tx = {
        let (pubkey, key) = crypto::gen_keypair_from_seed(&crypto::Seed::from_slice(
            &crypto::hash(b"correct horse battery staple")[..],
        ).unwrap());
        assert_eq!(pubkey, PublicKey::from_hex(ADMIN_KEY).unwrap());

        TxReset::new(&pubkey, &key)
    };

    let snapshot = testkit.probe(tx.clone());
    let schema = CounterSchema::new(&snapshot);
    assert_eq!(schema.count(), Some(6));
    // Check that data is not persisted
    let snapshot = testkit.snapshot();
    let schema = CounterSchema::new(&snapshot);
    assert_eq!(schema.count(), None);

    // Check dependency of the resulting snapshot on tx ordering
    let snapshot = testkit.probe_all(txvec![tx.clone(), admin_tx.clone()]);
    let schema = CounterSchema::new(&snapshot);
    assert_eq!(schema.count(), Some(0));
    let snapshot = testkit.probe_all(txvec![admin_tx.clone(), tx.clone()]);
    let schema = CounterSchema::new(&snapshot);
    assert_eq!(schema.count(), Some(6));
    // Check that data is (still) not persisted
    let snapshot = testkit.snapshot();
    let schema = CounterSchema::new(&snapshot);
    assert_eq!(schema.count(), None);

    api.send(other_tx);
    testkit.create_block();
    let snapshot = testkit.snapshot();
    let schema = CounterSchema::new(&snapshot);
    assert_eq!(schema.count(), Some(10));

    let snapshot = testkit.probe(tx.clone());
    let schema = CounterSchema::new(&snapshot);
    assert_eq!(schema.count(), Some(16));
    // Check that data is not persisted
    let snapshot = testkit.snapshot();
    let schema = CounterSchema::new(&snapshot);
    assert_eq!(schema.count(), Some(10));

    // Check dependency of the resulting snapshot on tx ordering
    let snapshot = testkit.probe_all(txvec![tx.clone(), admin_tx.clone()]);
    let schema = CounterSchema::new(&snapshot);
    assert_eq!(schema.count(), Some(0));
    let snapshot = testkit.probe_all(txvec![admin_tx.clone(), tx.clone()]);
    let schema = CounterSchema::new(&snapshot);
    assert_eq!(schema.count(), Some(6));
    // Check that data is (still) not persisted
    let snapshot = testkit.snapshot();
    let schema = CounterSchema::new(&snapshot);
    assert_eq!(schema.count(), Some(10));
}

#[test]
fn test_probe_duplicate_tx() {
    //! Checks that committed transactions do not change the blockchain state when probed.

    let (mut testkit, api) = init_testkit();
    let tx = inc_count(&api, 5);

    let snapshot = testkit.probe(tx.clone());
    let schema = CounterSchema::new(&snapshot);
    assert_eq!(schema.count(), Some(5));

    testkit.create_block();

    let snapshot = testkit.probe(tx.clone());
    let schema = CounterSchema::new(&snapshot);
    assert_eq!(schema.count(), Some(5));

    // Check the mixed case, when some probed transactions are committed and some are not
    let other_tx = inc_count(&api, 7);
    let snapshot = testkit.probe_all(txvec![tx, other_tx]);
    let schema = CounterSchema::new(&snapshot);
    assert_eq!(schema.count(), Some(12));
}

#[test]
fn test_snapshot_comparison() {
    let (mut testkit, api) = init_testkit();

    let tx = {
        let (pubkey, key) = crypto::gen_keypair();
        TxIncrement::new(&pubkey, 5, &key)
    };
    testkit
        .probe(tx.clone())
        .compare(testkit.snapshot())
        .map(CounterSchema::new)
        .map(CounterSchema::count)
        .assert_before("Counter does not exist", Option::is_none)
        .assert_after("Counter has been set", |&c| c == Some(5));

    api.send(tx);
    testkit.create_block();

    let other_tx = {
        let (pubkey, key) = crypto::gen_keypair();
        TxIncrement::new(&pubkey, 3, &key)
    };
    testkit
        .probe(other_tx.clone())
        .compare(testkit.snapshot())
        .map(CounterSchema::new)
        .map(CounterSchema::count)
        .map(|&c| c.unwrap())
        .assert("Counter has increased", |&old, &new| new == old + 3);
}

#[test]
#[should_panic(expected = "Counter has increased")]
fn test_snapshot_comparison_panic() {
    let (mut testkit, api) = init_testkit();

    let tx = {
        let (pubkey, key) = crypto::gen_keypair();
        TxIncrement::new(&pubkey, 5, &key)
    };

    api.send(tx.clone());
    testkit.create_block();

    // The assertion fails because the transaction is already committed by now
    testkit
        .probe(tx.clone())
        .compare(testkit.snapshot())
        .map(CounterSchema::new)
        .map(CounterSchema::count)
        .map(|&c| c.unwrap())
        .assert("Counter has increased", |&old, &new| new == old + tx.by());
}

#[test]
fn test_explorer_blocks() {
    use exonum::blockchain::Block;
    use exonum::helpers::Height;

    let (mut testkit, api) = init_testkit();

    let blocks: Vec<Block> = api.get(ApiKind::Explorer, "v1/blocks?count=10");
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].height(), Height(0));
    assert_eq!(*blocks[0].prev_hash(), crypto::Hash::default());

    // Check empty block creation
    testkit.create_block();

    let blocks: Vec<Block> = api.get(ApiKind::Explorer, "v1/blocks?count=10");
    assert_eq!(blocks.len(), 2);
    assert_eq!(blocks[0].height(), Height(1));
    assert_eq!(*blocks[0].prev_hash(), blocks[1].hash());
    assert_eq!(blocks[0].tx_count(), 0);
    assert_eq!(blocks[1].height(), Height(0));
    assert_eq!(*blocks[1].prev_hash(), crypto::Hash::default());

    let blocks: Vec<Block> = api.get(
        ApiKind::Explorer,
        "v1/blocks?count=10&skip_empty_blocks=true",
    );
    assert_eq!(blocks.len(), 0);

    let tx = {
        let (pubkey, key) = crypto::gen_keypair();
        TxIncrement::new(&pubkey, 5, &key)
    };
    testkit.api().send(tx.clone());
    testkit.create_block(); // height == 2

    let blocks: Vec<Block> = api.get(ApiKind::Explorer, "v1/blocks?count=10");
    assert_eq!(blocks.len(), 3);
    assert_eq!(blocks[0].height(), Height(2));
    assert_eq!(*blocks[0].prev_hash(), blocks[1].hash());
    assert_eq!(blocks[0].tx_count(), 1);
    assert_eq!(*blocks[0].tx_hash(), tx.hash());

    let blocks: Vec<Block> = api.get(
        ApiKind::Explorer,
        "v1/blocks?count=10&skip_empty_blocks=true",
    );
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].height(), Height(2));

    testkit.create_block(); // height == 3
    testkit.create_block(); // height == 4

    let blocks: Vec<Block> = api.get(
        ApiKind::Explorer,
        "v1/blocks?count=10&skip_empty_blocks=true",
    );
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].height(), Height(2));

    let tx = {
        let (pubkey, key) = crypto::gen_keypair();
        TxIncrement::new(&pubkey, 5, &key)
    };
    testkit.api().send(tx.clone());
    testkit.create_block(); // height == 5

    // Check block filtering
    let blocks: Vec<Block> = api.get(
        ApiKind::Explorer,
        "v1/blocks?count=3&skip_empty_blocks=true",
    );
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].height(), Height(5));
    let blocks: Vec<Block> = api.get(
        ApiKind::Explorer,
        "v1/blocks?count=4&skip_empty_blocks=true",
    );
    assert_eq!(blocks.len(), 2);
    assert_eq!(blocks[0].height(), Height(5));
    assert_eq!(blocks[1].height(), Height(2));

    // Check `latest` param
    let blocks: Vec<Block> = api.get(
        ApiKind::Explorer,
        "v1/blocks?count=10&skip_empty_blocks=true&latest=4",
    );
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].height(), Height(2));
}

#[test]
fn test_explorer_single_block() {
    use std::collections::HashSet;
    use exonum::explorer::BlockInfo;
    use exonum::helpers::Height;

    let mut testkit = TestKitBuilder::validator()
        .with_validators(4)
        .with_service(CounterService)
        .create();
    let api = testkit.api();

    assert_eq!(testkit.majority_count(), 3);

    let info: BlockInfo = api.get(ApiKind::Explorer, "v1/blocks/0");
    assert_eq!(info.block.height(), Height(0));
    assert_eq!(*info.block.prev_hash(), crypto::Hash::default());
    assert_eq!(info.txs, vec![]);

    let tx = {
        let (pubkey, key) = crypto::gen_keypair();
        TxIncrement::new(&pubkey, 5, &key)
    };
    testkit.api().send(tx.clone());
    testkit.create_block(); // height == 1

    let info: BlockInfo = api.get(ApiKind::Explorer, "v1/blocks/1");
    assert_eq!(info.block.height(), Height(1));
    assert_eq!(info.block.tx_count(), 1);
    assert_eq!(*info.block.tx_hash(), tx.hash());
    assert_eq!(info.txs, vec![tx.hash()]);

    let mut validators = HashSet::new();
    for precommit in &info.precommits {
        assert_eq!(precommit.height(), Height(1));
        assert_eq!(*precommit.block_hash(), info.block.hash());
        let pk = testkit
            .network()
            .consensus_public_key_of(precommit.validator())
            .expect("Cannot find validator id");
        assert!(precommit.verify_signature(pk));
        validators.insert(precommit.validator());
    }

    assert!(validators.len() >= testkit.majority_count());
}

#[test]
fn test_system_transaction() {
    use exonum::explorer::{BlockInfo, TxInfo as CommittedTxInfo};
    use exonum::helpers::Height;

    // Analogs of structures defined by the system API handler.
    #[derive(Deserialize)]
    struct MemPoolTxInfo {
        content: serde_json::Value,
    }

    #[derive(Deserialize)]
    #[serde(tag = "type")]
    enum TxInfo {
        Unknown,
        MemPool(MemPoolTxInfo),
        Committed(CommittedTxInfo),
    }


    let mut testkit = TestKitBuilder::validator()
        .with_validators(4)
        .with_service(CounterService)
        .create();
    let api = testkit.api();

    let tx = {
        let (pubkey, key) = crypto::gen_keypair();
        TxIncrement::new(&pubkey, 5, &key)
    };

    let info: TxInfo = api.get_err(
        ApiKind::System,
        &format!("v1/transactions/{}", &tx.hash().to_string()),
    );
    match info {
        TxInfo::Unknown => {}
        _ => panic!("Transaction should be unknown to the node"),
    }

    api.send(tx.clone());
    testkit.poll_events();

    let info: TxInfo = api.get(
        ApiKind::System,
        &format!("v1/transactions/{}", &tx.hash().to_string()),
    );
    if let TxInfo::MemPool(info) = info {
        assert_eq!(info.content, tx.serialize_field().unwrap());
    } else {
        panic!("Transaction should be in the mempool");
    }

    testkit.create_block();
    let info: TxInfo = api.get(
        ApiKind::System,
        &format!("v1/transactions/{}", &tx.hash().to_string()),
    );
    if let TxInfo::Committed(info) = info {
        assert_eq!(info.content, tx.serialize_field().unwrap());
        assert_eq!(info.location.block_height(), Height(1));
        assert_eq!(info.location.position_in_block(), 0);

        let block: BlockInfo = api.get(ApiKind::Explorer, "v1/blocks/1");
        let block = block.block;
        assert!(
            info.proof_to_block_merkle_root
                .validate(*block.tx_hash(), u64::from(block.tx_count()))
                .is_ok()
        );
    } else {
        panic!("Transaction should be committed");
    }
}
