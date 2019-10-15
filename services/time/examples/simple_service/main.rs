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

use exonum_merkledb::{IndexAccess, ObjectHash, ProofMapIndex, Snapshot};

use chrono::{DateTime, Duration, TimeZone, Utc};
use exonum::{
    blockchain::ExecutionError,
    crypto::{gen_keypair, Hash, PublicKey, SecretKey},
    helpers::Height,
    messages::Verified,
    runtime::{
        rust::{Service, Transaction, TransactionContext},
        AnyTx, InstanceDescriptor, InstanceId,
    },
};
use exonum_proto_derive::ProtobufConvert;
use exonum_testkit::{InstanceCollection, TestKitBuilder};
use exonum_time::{
    schema::TimeSchema,
    time_provider::{MockTimeProvider, TimeProvider},
    TimeServiceFactory,
};

use std::sync::Arc;

mod proto;

/// Time oracle instance ID.
const TIME_SERVICE_ID: InstanceId = 112;
/// Time oracle instance name.
const TIME_SERVICE_NAME: &str = "time-oracle";
/// Marker service ID.
const SERVICE_ID: InstanceId = 128;
/// Marker service name.
const SERVICE_NAME: &str = "marker";

/// Marker service database schema.
#[derive(Debug)]
pub struct MarkerSchema<'a, T> {
    access: T,
    service_name: &'a str,
}

impl<'a, T: IndexAccess> MarkerSchema<'a, T> {
    /// Constructs schema for the given `snapshot`.
    pub fn new(service_name: &'a str, access: T) -> Self {
        MarkerSchema {
            service_name,
            access,
        }
    }

    fn index_name(&self, name: &str) -> String {
        [self.service_name, ".", name].concat()
    }

    /// Returns the table mapping `i32` value to public keys authoring marker transactions.
    pub fn marks(&self) -> ProofMapIndex<T, PublicKey, i32> {
        ProofMapIndex::new(self.index_name("marks"), self.access.clone())
    }

    /// Returns hashes for stored table.
    pub fn state_hash(&self) -> Vec<Hash> {
        vec![self.marks().object_hash()]
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, ProtobufConvert, BinaryValue, ObjectHash)]
#[exonum(pb = "proto::TxMarker")]
/// Transaction, which must be executed no later than the specified time (field `time`).
pub struct TxMarker {
    mark: i32,
    time: DateTime<Utc>,
}

impl TxMarker {
    fn signed(
        mark: i32,
        time: DateTime<Utc>,
        public_key: &PublicKey,
        secret_key: &SecretKey,
    ) -> Verified<AnyTx> {
        Self { mark, time }.sign(SERVICE_ID, *public_key, secret_key)
    }
}

#[exonum_service]
pub trait MarkerInterface {
    fn mark(&self, context: TransactionContext, arg: TxMarker) -> Result<(), ExecutionError>;
}

#[derive(Debug, ServiceFactory)]
#[exonum(
    artifact_name = "marker",
    artifact_version = "0.1.0",
    proto_sources = "proto",
    implements("MarkerInterface")
)]
struct MarkerService;

impl MarkerInterface for MarkerService {
    fn mark(&self, context: TransactionContext, arg: TxMarker) -> Result<(), ExecutionError> {
        let author = context
            .caller()
            .as_transaction()
            .expect("Wrong `TxMarker` initiator")
            .1;

        let time = TimeSchema::new(TIME_SERVICE_NAME, context.fork())
            .time()
            .get();
        match time {
            Some(current_time) if current_time <= arg.time => {
                let schema = MarkerSchema::new(context.instance.name, context.fork());
                schema.marks().put(&author, arg.mark);
            }
            _ => {}
        }
        Ok(())
    }
}

impl Service for MarkerService {
    fn state_hash(&self, descriptor: InstanceDescriptor, snapshot: &dyn Snapshot) -> Vec<Hash> {
        MarkerSchema::new(descriptor.name, snapshot).state_hash()
    }
}

fn main() {
    let mock_provider = Arc::new(MockTimeProvider::default());
    // Create testkit for network with one validator.
    let mut testkit = TestKitBuilder::validator()
        .with_rust_service(
            InstanceCollection::new(TimeServiceFactory::with_provider(
                mock_provider.clone() as Arc<dyn TimeProvider>
            ))
            .with_instance(TIME_SERVICE_ID, TIME_SERVICE_NAME, ()),
        )
        .with_rust_service(InstanceCollection::new(MarkerService).with_instance(
            SERVICE_ID,
            SERVICE_NAME,
            (),
        ))
        .create();

    mock_provider.set_time(Utc.timestamp(10, 0));
    testkit.create_blocks_until(Height(2));

    let snapshot = testkit.snapshot();
    let time_schema = TimeSchema::new(TIME_SERVICE_NAME, &snapshot);
    assert_eq!(
        time_schema.time().get().map(|time| time),
        Some(mock_provider.time())
    );

    let keypair1 = gen_keypair();
    let keypair2 = gen_keypair();
    let keypair3 = gen_keypair();
    let tx1 = TxMarker::signed(1, mock_provider.time(), &keypair1.0, &keypair1.1);
    let tx2 = TxMarker::signed(
        2,
        mock_provider.time() + Duration::seconds(10),
        &keypair2.0,
        &keypair2.1,
    );
    let tx3 = TxMarker::signed(
        3,
        mock_provider.time() - Duration::seconds(5),
        &keypair3.0,
        &keypair3.1,
    );
    testkit.create_block_with_transactions(txvec![tx1, tx2, tx3]);

    let snapshot = testkit.snapshot();
    let schema = MarkerSchema::new(SERVICE_NAME, &snapshot);
    assert_eq!(schema.marks().get(&keypair1.0), Some(1));
    assert_eq!(schema.marks().get(&keypair2.0), Some(2));
    assert_eq!(schema.marks().get(&keypair3.0), None);

    let tx4 = TxMarker::signed(4, Utc.timestamp(15, 0), &keypair3.0, &keypair3.1);
    testkit.create_block_with_transactions(txvec![tx4]);

    let snapshot = testkit.snapshot();
    let schema = MarkerSchema::new(SERVICE_NAME, &snapshot);
    assert_eq!(schema.marks().get(&keypair3.0), Some(4));
}
