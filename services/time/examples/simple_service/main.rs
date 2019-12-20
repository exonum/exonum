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

use chrono::{DateTime, Duration, TimeZone, Utc};
use exonum::{
    crypto::{gen_keypair, PublicKey, SecretKey},
    helpers::Height,
    merkledb::{access::Access, ProofMapIndex},
    messages::Verified,
    runtime::{
        rust::{CallContext, Service, ServiceFactory, Transaction},
        AnyTx, ExecutionError, InstanceId, SnapshotExt,
    },
};
use exonum_derive::*;
use exonum_proto::ProtobufConvert;
use exonum_testkit::TestKitBuilder;
use serde_derive::*;

use exonum_time::{
    schema::TimeSchema,
    time_provider::{MockTimeProvider, TimeProvider},
    TimeServiceFactory,
};
use std::sync::Arc;

mod proto;

/// The argument of the `MarkerInterface::mark` method.
#[derive(Clone, Debug)]
#[derive(Serialize, Deserialize)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "proto::TxMarker")]
pub struct TxMarker {
    mark: i32,
    time: DateTime<Utc>,
}

/// Marker service transactions interface definition.
#[exonum_interface]
pub trait MarkerTransactions {
    /// Transaction, which must be executed no later than the specified time (field `time`).
    fn mark(&self, context: CallContext<'_>, arg: TxMarker) -> Result<(), ExecutionError>;
}

#[derive(Debug, ServiceDispatcher, ServiceFactory)]
#[service_factory(
    artifact_name = "marker",
    artifact_version = "0.1.0",
    proto_sources = "proto"
)]
#[service_dispatcher(implements("MarkerTransactions"))]
struct MarkerService;

/// Marker service database schema.
#[derive(Debug, FromAccess)]
pub struct MarkerSchema<T: Access> {
    pub marks: ProofMapIndex<T::Base, PublicKey, i32>,
}

impl MarkerTransactions for MarkerService {
    fn mark(&self, context: CallContext<'_>, arg: TxMarker) -> Result<(), ExecutionError> {
        let author = context
            .caller()
            .author()
            .expect("Wrong `TxMarker` initiator");

        let data = context.data();
        let time_service_data = data
            .for_service(TIME_SERVICE_NAME)
            .expect("No time service data");
        let time = TimeSchema::new(time_service_data).time.get();
        match time {
            Some(current_time) if current_time <= arg.time => {
                let mut schema = MarkerSchema::new(context.service_data());
                schema.marks.put(&author, arg.mark);
            }
            _ => {}
        }
        Ok(())
    }
}

impl Service for MarkerService {}

// Several helpers for testkit.

/// Time oracle instance ID.
const TIME_SERVICE_ID: InstanceId = 112;
/// Time oracle instance name.
const TIME_SERVICE_NAME: &str = "time-oracle";
/// Marker service ID.
const SERVICE_ID: InstanceId = 128;
/// Marker service name.
const SERVICE_NAME: &str = "marker";

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

fn main() {
    let mock_provider = Arc::new(MockTimeProvider::default());
    // Create testkit for network with one validator.
    let time_service =
        TimeServiceFactory::with_provider(mock_provider.clone() as Arc<dyn TimeProvider>);
    let time_service_artifact = time_service.artifact_id();
    let marker_service = MarkerService;
    let marker_service_artifact = marker_service.artifact_id();

    let mut testkit = TestKitBuilder::validator()
        .with_artifact(time_service_artifact.clone())
        .with_instance(
            time_service_artifact.into_default_instance(TIME_SERVICE_ID, TIME_SERVICE_NAME),
        )
        .with_rust_service(time_service)
        .with_artifact(marker_service_artifact.clone())
        .with_instance(marker_service_artifact.into_default_instance(SERVICE_ID, SERVICE_NAME))
        .with_rust_service(marker_service)
        .create();

    mock_provider.set_time(Utc.timestamp(10, 0));
    testkit.create_blocks_until(Height(2));

    let snapshot = testkit.snapshot();
    let snapshot = snapshot.for_service(TIME_SERVICE_NAME).unwrap();
    let time_schema = TimeSchema::new(snapshot);
    assert_eq!(
        time_schema.time.get().map(|time| time),
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
    testkit.create_block_with_transactions(vec![tx1, tx2, tx3]);

    let snapshot = testkit.snapshot();
    let schema = MarkerSchema::new(snapshot.for_service(SERVICE_NAME).unwrap());
    assert_eq!(schema.marks.get(&keypair1.0), Some(1));
    assert_eq!(schema.marks.get(&keypair2.0), Some(2));
    assert_eq!(schema.marks.get(&keypair3.0), None);

    let tx4 = TxMarker::signed(4, Utc.timestamp(15, 0), &keypair3.0, &keypair3.1);
    testkit.create_block_with_transactions(vec![tx4]);

    let snapshot = testkit.snapshot();
    let schema = MarkerSchema::new(snapshot.for_service(SERVICE_NAME).unwrap());
    assert_eq!(schema.marks.get(&keypair3.0), Some(4));
}
