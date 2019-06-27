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

#[macro_use]
extern crate pretty_assertions;

use exonum::api::node::private::AddAuditorRequest;
use exonum::api::node::public::system::{KeyInfo, SharedConfiguration};
use exonum::node::{ConnectInfo, ExternalMessage};
use exonum::{
    api::node::{
        private::NodeInfo,
        public::system::{ConsensusStatus, HealthCheckInfo, StatsInfo},
    },
    crypto::gen_keypair,
    helpers::user_agent,
    messages::PROTOCOL_MAJOR_VERSION,
};
use exonum_testkit::{ApiKind, TestKitBuilder};

#[test]
fn healthcheck() {
    // This test checks whether the endpoint returns expected result and correctness of
    // serialize.
    // Expected:
    // consensus - enabled
    // connectivity - not connected, due to testkit unable to emulate nodes properly.
    let testkit = TestKitBuilder::validator().with_validators(2).create();
    let api = testkit.api();

    let info: HealthCheckInfo = api.public(ApiKind::System).get("v1/healthcheck").unwrap();
    let expected = HealthCheckInfo {
        consensus_status: ConsensusStatus::Enabled,
        connected_peers: 0,
    };
    assert_eq!(info, expected);
}

#[test]
fn stats() {
    let testkit = TestKitBuilder::validator().with_validators(2).create();
    let api = testkit.api();

    let info: StatsInfo = api.public(ApiKind::System).get("v1/stats").unwrap();
    let expected = StatsInfo {
        tx_pool_size: 0,
        tx_count: 0,
    };
    assert_eq!(info, expected);
}

#[test]
fn user_agent_info() {
    let testkit = TestKitBuilder::validator().with_validators(2).create();
    let api = testkit.api();

    let info: String = api.public(ApiKind::System).get("v1/user_agent").unwrap();
    let expected = user_agent::get();
    assert_eq!(info, expected);
}

#[test]
fn network() {
    let testkit = TestKitBuilder::validator().with_validators(2).create();
    let api = testkit.api();

    let info: NodeInfo = api.private(ApiKind::System).get("v1/network").unwrap();
    assert!(info.core_version.is_some());
    assert_eq!(info.protocol_version, PROTOCOL_MAJOR_VERSION);
    assert!(info.services.is_empty());
}

#[test]
fn shutdown() {
    let testkit = TestKitBuilder::validator().with_validators(2).create();
    let api = testkit.api();

    assert_eq!(
        api.private(ApiKind::System)
            .post::<()>("v1/shutdown")
            .unwrap(),
        ()
    );
}

#[test]
fn rebroadcast() {
    let testkit = TestKitBuilder::validator().with_validators(2).create();
    let api = testkit.api();

    assert_eq!(
        api.private(ApiKind::System)
            .post::<()>("v1/rebroadcast")
            .unwrap(),
        ()
    )
}

#[test]
fn service_key() {
    let testkit = TestKitBuilder::validator().with_validators(2).create();
    let api = testkit.api();

    let info: KeyInfo = api.public(ApiKind::System).get("v1/service_key").unwrap();

    assert_eq!(info.pub_key, *testkit.us().service_keypair().0)
}

#[test]
#[should_panic(expected = "Peer with this public key not found")]
fn remote_config() {
    let testkit = TestKitBuilder::validator().with_validators(2).create();
    let api = testkit.api();

    api.public(ApiKind::System)
        .query(&KeyInfo {
            pub_key: gen_keypair().0,
        })
        .get::<SharedConfiguration>("v1/remote_config")
        .unwrap();
}

#[test]
fn auditor_add() {
    let mut testkit = TestKitBuilder::validator().with_validators(2).create();
    let api = testkit.api();

    let (pub_key, _) = gen_keypair();

    api.private(ApiKind::System)
        .query(&AddAuditorRequest {
            address: "localhost:5333".to_string(),
            public_key: pub_key,
            connect_all: true,
            validators: vec![],
        })
        .post::<()>("v1/auditor/add")
        .unwrap();

    testkit.poll_events();

    testkit
        .received_messages()
        .iter()
        .find(|msg| match msg {
            ExternalMessage::AuditorAdd(msg) => {
                msg.connect_all && msg.address == "localhost:5333" && msg.public_key == pub_key
            }
            _ => false,
        })
        .unwrap();
}

#[test]
fn peer_add() {
    let mut testkit = TestKitBuilder::validator().with_validators(2).create();
    let api = testkit.api();

    let (pub_key, _) = gen_keypair();

    api.private(ApiKind::System)
        .query(&ConnectInfo {
            address: "localhost:5333".to_string(),
            public_key: pub_key,
        })
        .post::<()>("v1/peers")
        .unwrap();

    testkit.poll_events();

    testkit
        .received_messages()
        .iter()
        .find(|msg| match msg {
            ExternalMessage::PeerAdd(msg) => {
                msg.address == "localhost:5333" && msg.public_key == pub_key
            }
            _ => false,
        })
        .unwrap();
}
