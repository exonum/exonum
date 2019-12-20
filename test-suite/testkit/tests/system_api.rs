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

use exonum::{
    api::node::{
        private::NodeInfo,
        public::system::{ConsensusStatus, HealthCheckInfo, StatsInfo},
    },
    helpers::user_agent,
};
use exonum_testkit::{ApiKind, TestKitBuilder};

#[test]
fn healthcheck() {
    // This test checks whether the endpoint returns expected result and correctness of
    // serialize. Expected results:
    //
    // - consensus - enabled
    // - connectivity - not connected, due to testkit unable to emulate nodes properly.
    let mut testkit = TestKitBuilder::validator().with_validators(2).create();
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
    let mut testkit = TestKitBuilder::validator().with_validators(2).create();
    let api = testkit.api();
    let info: StatsInfo = api.public(ApiKind::System).get("v1/stats").unwrap();
    let expected = StatsInfo {
        tx_pool_size: 0,
        tx_count: 0,
        tx_cache_size: 0,
    };
    assert_eq!(info, expected);
}

#[test]
fn user_agent_info() {
    let mut testkit = TestKitBuilder::validator().with_validators(2).create();
    let api = testkit.api();
    let info: String = api.public(ApiKind::System).get("v1/user_agent").unwrap();
    let expected = user_agent();
    assert_eq!(info, expected);
}

#[test]
fn network() {
    let mut testkit = TestKitBuilder::validator().with_validators(2).create();
    let api = testkit.api();
    let info: NodeInfo = api.private(ApiKind::System).get("v1/network").unwrap();
    assert!(info.core_version.is_some());
}

#[test]
fn shutdown() {
    let mut testkit = TestKitBuilder::validator().with_validators(2).create();
    let api = testkit.api();
    assert_eq!(
        api.private(ApiKind::System)
            .post::<()>("v1/shutdown")
            .unwrap(),
        ()
    );
}
