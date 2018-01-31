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

//! Service, which uses the time oracle.

#[macro_use]
extern crate exonum;
extern crate exonum_time;
#[macro_use]
extern crate exonum_testkit;
extern crate serde_json;
extern crate serde;

use std::time::{UNIX_EPOCH, SystemTime, Duration};
use exonum::blockchain::{Service, Transaction};
use exonum::crypto::{gen_keypair, Hash, PublicKey};
use exonum::encoding;
use exonum::helpers::Height;
use exonum::messages::{Message, RawTransaction};
use exonum::storage::{Fork, ProofMapIndex, Snapshot};
use exonum_time::{TimeService, TimeSchema, MockTimeProvider};
use exonum_testkit::TestKitBuilder;

/// Marker service id.
const SERVICE_ID: u16 = 128;
/// Marker service name.
const SERVICE_NAME: &str = "marker";
/// `TxMarker` transaction id.
const TX_MARKER_ID: u16 = 0;

/// Marker service database schema.
#[derive(Debug)]
pub struct MarkerSchema<T> {
    view: T,
}

impl<T: AsRef<Snapshot>> MarkerSchema<T> {
    /// Constructs schema for the given `snapshot`.
    pub fn new(view: T) -> Self {
        MarkerSchema { view }
    }

    /// Returns the table that stores `i32` value for every node.
    pub fn marks(&self) -> ProofMapIndex<&Snapshot, PublicKey, i32> {
        ProofMapIndex::new(format!("{}.marks", SERVICE_NAME), self.view.as_ref())
    }

    /// Returns hashes for stored table.
    pub fn state_hash(&self) -> Vec<Hash> {
        vec![self.marks().root_hash()]
    }
}


impl<'a> MarkerSchema<&'a mut Fork> {
    /// Mutable reference to the ['marks'][1] index.
    ///
    /// [1]: struct.MarkerSchema.html#method.marks
    pub fn marks_mut(&mut self) -> ProofMapIndex<&mut Fork, PublicKey, i32> {
        ProofMapIndex::new(format!("{}.marks", SERVICE_NAME), self.view)
    }
}

message! {
    /// Transaction, which must be executed no later than the specified time (field `time`).
    struct TxMarker {
        const TYPE = SERVICE_ID;
        const ID = TX_MARKER_ID;

        from: &PublicKey,
        mark: i32,
        time: SystemTime,
    }
}

impl Transaction for TxMarker {
    fn verify(&self) -> bool {
        self.verify_signature(self.from())
    }

    fn execute(&self, view: &mut Fork) {
        let time = TimeSchema::new(&view).time().get();
        match time {
            Some(ref current_time) if current_time.time() <= self.time() => {
                let mut schema = MarkerSchema::new(view);
                schema.marks_mut().put(self.from(), self.mark());
            }
            _ => {}
        }
    }
}

struct MarkerService;

impl Service for MarkerService {
    fn service_name(&self) -> &'static str {
        SERVICE_NAME
    }

    fn state_hash(&self, snapshot: &Snapshot) -> Vec<Hash> {
        let schema = MarkerSchema::new(snapshot);
        schema.state_hash()
    }

    fn service_id(&self) -> u16 {
        SERVICE_ID
    }

    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, encoding::Error> {
        match raw.message_type() {
            TX_MARKER_ID => Ok(Box::new(TxMarker::from_raw(raw)?)),
            _ => {
                let error =
                    encoding::Error::IncorrectMessageType { message_type: raw.message_type() };
                Err(error)
            }
        }
    }
}

fn main() {
    let mock_provider = MockTimeProvider::default();
    // Create testkit for network with one validator.
    let mut testkit = TestKitBuilder::validator()
        .with_service(MarkerService)
        .with_service(TimeService::with_provider(mock_provider.clone()))
        .create();

    mock_provider.set_time(UNIX_EPOCH + Duration::new(10, 0));
    testkit.create_blocks_until(Height(2));

    let snapshot = testkit.snapshot();
    let time_schema = TimeSchema::new(&snapshot);
    assert_eq!(
        time_schema.time().get().map(|time| time.time()),
        Some(mock_provider.time())
    );

    let keypair1 = gen_keypair();
    let keypair2 = gen_keypair();
    let keypair3 = gen_keypair();
    let tx1 = TxMarker::new(&keypair1.0, 1, mock_provider.time(), &keypair1.1);
    let tx2 = TxMarker::new(
        &keypair2.0,
        2,
        UNIX_EPOCH + Duration::new(20, 0),
        &keypair2.1,
    );
    let tx3 = TxMarker::new(
        &keypair3.0,
        3,
        UNIX_EPOCH + Duration::new(5, 0),
        &keypair3.1,
    );
    testkit.create_block_with_transactions(txvec![tx1, tx2, tx3]);

    let snapshot = testkit.snapshot();
    let schema = MarkerSchema::new(snapshot);
    assert_eq!(schema.marks().get(&keypair1.0), Some(1));
    assert_eq!(schema.marks().get(&keypair2.0), Some(2));
    assert_eq!(schema.marks().get(&keypair3.0), None);

    let tx4 = TxMarker::new(
        &keypair3.0,
        4,
        UNIX_EPOCH + Duration::new(15, 0),
        &keypair3.1,
    );
    testkit.create_block_with_transactions(txvec![tx4]);

    let snapshot = testkit.snapshot();
    let schema = MarkerSchema::new(snapshot);
    assert_eq!(schema.marks().get(&keypair3.0), Some(4));
}
