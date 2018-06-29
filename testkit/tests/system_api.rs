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

extern crate exonum;
extern crate exonum_testkit;
#[macro_use]
extern crate pretty_assertions;

use exonum::{
    api::node::{
        private::NodeInfo, public::system::{ConsensusStatusInfo, HealthCheckInfo},
    },
    helpers::user_agent, messages::PROTOCOL_MAJOR_VERSION,
};
use exonum_testkit::{ApiKind, TestKitBuilder};

#[test]
fn test_healthcheck_connectivity_false() {
    let testkit = TestKitBuilder::validator().with_validators(2).create();
    let api = testkit.api();

    let info: HealthCheckInfo = api.public(ApiKind::System).get("v1/healthcheck").unwrap();
    let expected = HealthCheckInfo {
        connectivity: false,
    };
    assert_eq!(info, expected);
}

#[test]
fn test_user_agent_info() {
    let testkit = TestKitBuilder::validator().with_validators(2).create();
    let api = testkit.api();

    let info: String = api.public(ApiKind::System).get("v1/user_agent").unwrap();
    let expected = user_agent::get();
    assert_eq!(info, expected);
}

#[test]
fn test_network() {
    let testkit = TestKitBuilder::validator().with_validators(2).create();
    let api = testkit.api();

    let info: NodeInfo = api.private(ApiKind::System).get("v1/network").unwrap();
    assert!(info.core_version.is_some());
    assert_eq!(info.protocol_version, PROTOCOL_MAJOR_VERSION);
    assert!(info.services.is_empty());
}

#[test]
fn test_consensus_status_false() {
    let testkit = TestKitBuilder::validator().create();
    let api = testkit.api();

    let info: ConsensusStatusInfo = api.public(ApiKind::System)
        .get("v1/consensus_status")
        .unwrap();
    let expected = ConsensusStatusInfo { status: false };
    assert_eq!(info, expected);
}

#[test]
fn test_shutdown() {
    let testkit = TestKitBuilder::validator().with_validators(2).create();
    let api = testkit.api();

    assert_eq!(
        api.private(ApiKind::System)
            .post::<()>("v1/shutdown")
            .unwrap(),
        ()
    );
}
