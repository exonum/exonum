#[macro_use]
extern crate exonum;
#[macro_use]
extern crate exonum_testkit;
extern crate serde_json;

use exonum::crypto::{gen_keypair, PublicKey};
use exonum::blockchain::{Block, Schema, Service, Transaction};
use exonum::messages::{FromRaw, Message, RawTransaction};
use exonum::storage::Fork;
use exonum::encoding;
use exonum_testkit::{ApiKind, TestKitBuilder};

// Simple service implementation.

const SERVICE_ID: u16 = 512;
const TX_TIMESTAMP_ID: u16 = 0;

message! {
    struct TxTimestamp {
        const TYPE = SERVICE_ID;
        const ID = TX_TIMESTAMP_ID;
        const SIZE = 40;

        field from: &PublicKey [0 => 32]
        field msg: &str [32 => 40]
    }
}

struct TimestampingService;

impl Transaction for TxTimestamp {
    fn verify(&self) -> bool {
        self.verify_signature(self.from())
    }

    fn execute(&self, _fork: &mut Fork) {}

    fn info(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap()
    }
}

impl Service for TimestampingService {
    fn service_name(&self) -> &'static str {
        "timestamping"
    }

    fn service_id(&self) -> u16 {
        SERVICE_ID
    }

    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, encoding::Error> {
        let trans: Box<Transaction> = match raw.message_type() {
            TX_TIMESTAMP_ID => Box::new(TxTimestamp::from_raw(raw)?),
            _ => {
                return Err(encoding::Error::IncorrectMessageType {
                    message_type: raw.message_type(),
                });
            }
        };
        Ok(trans)
    }
}

fn main() {
    // Create testkit for network with four validators.
    let mut testkit = TestKitBuilder::validator()
        .with_validators(4)
        .with_service(TimestampingService)
        .create();
    // Create few transactions.
    let keypair = gen_keypair();
    let tx1 = TxTimestamp::new(&keypair.0, "Down To Earth", &keypair.1);
    let tx2 = TxTimestamp::new(&keypair.0, "Cry Over Spilt Milk", &keypair.1);
    let tx3 = TxTimestamp::new(&keypair.0, "Dropping Like Flies", &keypair.1);
    // Commit them into blockchain.
    testkit.create_block_with_transactions(txvec![tx1.clone(), tx2.clone(), tx3.clone()]);
    // Check results with schema.
    let snapshot = testkit.snapshot();
    let schema = Schema::new(&snapshot);
    assert!(schema.transactions().contains(&tx1.hash()));
    assert!(schema.transactions().contains(&tx2.hash()));
    assert!(schema.transactions().contains(&tx3.hash()));
    // Check results with api.
    let api = testkit.api();
    let blocks: Vec<Block> = api.get(ApiKind::Explorer, "v1/blocks?count=10");
    assert_eq!(blocks.len(), 2);
    api.get::<serde_json::Value>(
        ApiKind::System,
        &format!("v1/transactions/{}", tx1.hash().to_string()),
    );
}
