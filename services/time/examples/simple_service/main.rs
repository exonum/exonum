// Copyright 2019 The Exonum Team
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
extern crate exonum_testkit;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate exonum_derive;

use chrono::{DateTime, Duration, TimeZone, Utc};
use exonum::{
    blockchain::{ExecutionResult, Service, Transaction, TransactionContext, TransactionSet},
    crypto::{gen_keypair, Hash, PublicKey, SecretKey},
    helpers::Height,
    messages::{Message, RawTransaction, Signed},
    storage::{Fork, ProofMapIndex, Snapshot},
};
use exonum_testkit::TestKitBuilder;
use exonum_time::{schema::TimeSchema, time_provider::MockTimeProvider, TimeService};

mod proto;

/// Marker service id.
const SERVICE_ID: u16 = 128;
/// Marker service name.
const SERVICE_NAME: &str = "marker";

/// Marker service database schema.
#[derive(Debug)]
pub struct MarkerSchema<T> {
    view: T,
}

impl<T: AsRef<dyn Snapshot>> MarkerSchema<T> {
    /// Constructs schema for the given `snapshot`.
    pub fn new(view: T) -> Self {
        MarkerSchema { view }
    }

    /// Returns the table mapping `i32` value to public keys authoring marker transactions.
    pub fn marks(&self) -> ProofMapIndex<&dyn Snapshot, PublicKey, i32> {
        ProofMapIndex::new(format!("{}.marks", SERVICE_NAME), self.view.as_ref())
    }

    /// Returns hashes for stored table.
    pub fn state_hash(&self) -> Vec<Hash> {
        vec![self.marks().merkle_root()]
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

#[derive(Serialize, Deserialize, Debug, Clone, ProtobufConvert)]
#[exonum(pb = "proto::TxMarker")]
/// Transaction, which must be executed no later than the specified time (field `time`).
struct TxMarker {
    mark: i32,
    time: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Debug, Clone, TransactionSet)]
enum MarkerTransactions {
    TxMarker(TxMarker),
}

impl TxMarker {
    fn sign(
        mark: i32,
        time: DateTime<Utc>,
        public_key: &PublicKey,
        secret_key: &SecretKey,
    ) -> Signed<RawTransaction> {
        Message::sign_transaction(TxMarker { mark, time }, SERVICE_ID, *public_key, secret_key)
    }
}

impl Transaction for TxMarker {
    fn execute(&self, mut context: TransactionContext) -> ExecutionResult {
        let author = context.author();
        let view = context.fork();
        let time = TimeSchema::new(&view).time().get();
        match time {
            Some(current_time) if current_time <= self.time => {
                let mut schema = MarkerSchema::new(view);
                schema.marks_mut().put(&author, self.mark);
            }
            _ => {}
        }
        Ok(())
    }
}

struct MarkerService;

impl Service for MarkerService {
    fn service_name(&self) -> &str {
        SERVICE_NAME
    }

    fn state_hash(&self, snapshot: &dyn Snapshot) -> Vec<Hash> {
        let schema = MarkerSchema::new(snapshot);
        schema.state_hash()
    }

    fn service_id(&self) -> u16 {
        SERVICE_ID
    }

    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<dyn Transaction>, failure::Error> {
        let tx = MarkerTransactions::tx_from_raw(raw)?;
        Ok(tx.into())
    }
}

fn main() {
    let mock_provider = MockTimeProvider::default();
    // Create testkit for network with one validator.
    let mut testkit = TestKitBuilder::validator()
        .with_service(MarkerService)
        .with_service(TimeService::with_provider(mock_provider.clone()))
        .create();

    mock_provider.set_time(Utc.timestamp(10, 0));
    testkit.create_blocks_until(Height(2));

    let snapshot = testkit.snapshot();
    let time_schema = TimeSchema::new(&snapshot);
    assert_eq!(
        time_schema.time().get().map(|time| time),
        Some(mock_provider.time())
    );

    let keypair1 = gen_keypair();
    let keypair2 = gen_keypair();
    let keypair3 = gen_keypair();
    let tx1 = TxMarker::sign(1, mock_provider.time(), &keypair1.0, &keypair1.1);
    let tx2 = TxMarker::sign(
        2,
        mock_provider.time() + Duration::seconds(10),
        &keypair2.0,
        &keypair2.1,
    );
    let tx3 = TxMarker::sign(
        3,
        mock_provider.time() - Duration::seconds(5),
        &keypair3.0,
        &keypair3.1,
    );
    testkit.create_block_with_transactions(txvec![tx1, tx2, tx3]);

    let snapshot = testkit.snapshot();
    let schema = MarkerSchema::new(snapshot);
    assert_eq!(schema.marks().get(&keypair1.0), Some(1));
    assert_eq!(schema.marks().get(&keypair2.0), Some(2));
    assert_eq!(schema.marks().get(&keypair3.0), None);

    let tx4 = TxMarker::sign(4, Utc.timestamp(15, 0), &keypair3.0, &keypair3.1);
    testkit.create_block_with_transactions(txvec![tx4]);

    let snapshot = testkit.snapshot();
    let schema = MarkerSchema::new(snapshot);
    assert_eq!(schema.marks().get(&keypair3.0), Some(4));
}
