// Copyright 2020 The Exonum Team
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

use exonum_node::ExternalMessage;
use exonum_testkit::{ApiKind, TestKit, TestKitBuilder};
use pretty_assertions::assert_eq;

use exonum_system_api::{
    private::{ConsensusStatus, NodeInfo, NodeStats},
    SystemApiPlugin,
};

fn create_testkit() -> TestKit {
    TestKitBuilder::validator()
        .with_validators(2)
        .with_plugin(SystemApiPlugin)
        .build()
}

#[tokio::test]
async fn info() {
    // This test checks whether the endpoint returns expected result and correctness of
    // serialize. Expected results:
    //
    // - consensus - enabled
    // - connected_peers is empty, due to testkit unable to emulate nodes properly.
    let mut testkit = create_testkit();
    let api = testkit.api();

    let info: NodeInfo = api.private(ApiKind::System).get("v1/info").await.unwrap();
    assert_eq!(info.consensus_status, ConsensusStatus::Enabled);
    assert!(info.connected_peers.is_empty());
    assert_eq!(info.rust_version.major, 1);
}

#[tokio::test]
async fn stats() {
    let mut testkit = create_testkit();
    let api = testkit.api();
    let info: NodeStats = api.private(ApiKind::System).get("v1/stats").await.unwrap();
    assert_eq!(info.height, 0);
    assert_eq!(info.tx_cache_size, 0);
}

#[tokio::test]
async fn shutdown() {
    let mut testkit = create_testkit();
    let api = testkit.api();
    api.private(ApiKind::System)
        .post::<()>("v1/shutdown")
        .await
        .unwrap();
    let control_messages = testkit.poll_control_messages();
    match control_messages.as_slice() {
        [ExternalMessage::Shutdown] => {}
        _ => panic!("Unexpected control messages: {:?}", control_messages),
    }
}
