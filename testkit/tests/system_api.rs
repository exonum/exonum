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

use exonum::api::private::NodeInfo;
use exonum::api::public::HealthCheckInfo;
use exonum::helpers::user_agent;
use exonum::messages::PROTOCOL_MAJOR_VERSION;
use exonum_testkit::{ApiKind, TestKitBuilder};

#[test]
fn test_healthcheck_connectivity_false() {
    let testkit = TestKitBuilder::validator().with_validators(2).create();
    let api = testkit.api();
    let info: HealthCheckInfo = api.get(ApiKind::System, "v1/healthcheck");
    let expected = HealthCheckInfo {
        connectivity: false,
    };
    assert_eq!(info, expected);
}

#[test]
fn test_user_agent_info() {
    let testkit = TestKitBuilder::validator().with_validators(2).create();
    let api = testkit.api();
    let info: String = api.get(ApiKind::System, "v1/user_agent");
    let expected = user_agent::get();
    assert_eq!(info, expected);
}

#[test]
fn test_network() {
    let testkit = TestKitBuilder::validator().with_validators(2).create();
    let api = testkit.api();
    let info: NodeInfo = api.get_private(ApiKind::System, "/v1/network");

    assert_eq!(
        info.core_version,
        option_env!("CARGO_PKG_VERSION").map(|ver| ver.to_owned())
    );
    assert_eq!(info.protocol_version, PROTOCOL_MAJOR_VERSION);
    assert!(info.services.is_empty());
}
