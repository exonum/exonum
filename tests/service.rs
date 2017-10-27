#[macro_use]
extern crate exonum;
extern crate exonum_harness;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;

mod service {
    extern crate bodyparser;
    extern crate iron;
    extern crate router;

    use exonum::blockchain::{Blockchain, ApiContext, Service, Transaction};
    use exonum::messages::{RawTransaction, FromRaw, Message};
    use exonum::node::{ApiSender, TransactionSend};
    use exonum::storage::{Fork, Snapshot, Entry};
    use exonum::crypto::{PublicKey, Hash};
    use exonum::encoding;
    use exonum::api::{Api, ApiError};
    use self::iron::Handler;
    use self::iron::prelude::*;
    use self::router::Router;
    use serde_json;

    const SERVICE_ID: u16 = 1;
    const TX_INCREMENT_ID: u16 = 1;

    // "correct horse battery staple" brainwallet pubkey in Ed25519 with SHA-256 digest
    pub const ADMIN_KEY: &'static str = "506f27b1b4c2403f2602d663a059b026\
                                         2afd6a5bcda95a08dd96a4614a89f1b0";

    // // // // Schema // // // //

    struct CounterSchema<T> {
        view: T,
    }

    impl<T: AsRef<Snapshot>> CounterSchema<T> {
        fn new(view: T) -> Self {
            CounterSchema { view }
        }

        fn entry(&self) -> Entry<&Snapshot, u64> {
            Entry::new(vec![1], self.view.as_ref())
        }

        fn count(&self) -> Option<u64> {
            self.entry().get()
        }
    }

    impl<'a> CounterSchema<&'a mut Fork> {
        fn entry_mut(&mut self) -> Entry<&mut Fork, u64> {
            Entry::new(vec![1], self.view)
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
            use exonum::crypto::HexValue;
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

use exonum::blockchain::Service;
use exonum::crypto::{self, HexValue, PublicKey};
use exonum::helpers::Height;
use exonum::messages::Message;
use exonum_harness::{TestHarness, HarnessApi};
use service::{ADMIN_KEY, CounterService, TxIncrement, TxReset, TransactionResponse};

fn inc_count(api: &HarnessApi, by: u64) -> TxIncrement {
    let (pubkey, key) = crypto::gen_keypair();
    // Create a presigned transaction
    let tx = TxIncrement::new(&pubkey, by, &key);

    let tx_info: TransactionResponse = api.post("counter", "count", &tx);
    assert_eq!(tx_info.tx_hash, tx.hash());
    tx
}

#[test]
fn test_inc_count() {
    let services: Vec<Box<Service>> = vec![Box::new(CounterService)];
    let mut harness = TestHarness::with_services(services);
    let api = harness.api();
    inc_count(&api, 5);

    harness.create_block();

    // Check that the user indeed is persisted by the service
    let counter: u64 = api.get("counter", "count");
    assert_eq!(counter, 5);
}

#[test]
fn test_inc_count_with_multiple_transactions() {
    let services: Vec<Box<Service>> = vec![Box::new(CounterService)];
    let mut harness = TestHarness::with_services(services);
    let api = harness.api();

    for _ in 0..100 {
        inc_count(&api, 1);
        inc_count(&api, 2);
        inc_count(&api, 3);
        inc_count(&api, 4);

        harness.create_block();
    }

    assert_eq!(harness.state().height(), Height(101));
    let counter: u64 = api.get("counter", "count");
    assert_eq!(counter, 1_000);
}

#[test]
fn test_inc_count_with_manual_tx_control() {
    let services: Vec<Box<Service>> = vec![Box::new(CounterService)];
    let mut harness = TestHarness::with_services(services);
    let api = harness.api();
    let tx_a = inc_count(&api, 5);
    let tx_b = inc_count(&api, 3);

    // Empty block
    harness.create_block_with_transactions(&[]);
    let counter: u64 = api.get("counter", "count");
    assert_eq!(counter, 0);

    harness.create_block_with_transactions(&[tx_b.hash()]);
    let counter: u64 = api.get("counter", "count");
    assert_eq!(counter, 3);

    harness.create_block_with_transactions(&[tx_a.hash()]);
    let counter: u64 = api.get("counter", "count");
    assert_eq!(counter, 8);
}

#[test]
fn test_private_api() {
    let services: Vec<Box<Service>> = vec![Box::new(CounterService)];
    let mut harness = TestHarness::with_services(services);
    let api = harness.api();
    inc_count(&api, 5);
    inc_count(&api, 3);

    harness.create_block();
    let counter: u64 = api.get("counter", "count");
    assert_eq!(counter, 8);

    let (pubkey, key) = crypto::gen_keypair_from_seed(&crypto::Seed::from_slice(
        &crypto::hash(b"correct horse battery staple")[..],
    ).unwrap());
    assert_eq!(pubkey, PublicKey::from_hex(ADMIN_KEY).unwrap());

    let tx = TxReset::new(&pubkey, &key);
    let tx_info: TransactionResponse = api.post_private("counter", "reset", &tx);
    assert_eq!(tx_info.tx_hash, tx.hash());

    harness.create_block();
    let counter: u64 = api.get("counter", "count");
    assert_eq!(counter, 0);
}
