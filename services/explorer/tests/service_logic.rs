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

use exonum::{
    blockchain::CallInBlock,
    helpers::Height,
    runtime::{CallType, ErrorMatch, SUPERVISOR_INSTANCE_ID as SUPERVISOR_ID},
};
use exonum_rust_runtime::ServiceFactory;
use exonum_supervisor::{ConfigPropose, Supervisor, SupervisorInterface};
use exonum_testkit::TestKitBuilder;

use exonum_explorer_service::{Error, ExplorerFactory};

#[test]
#[should_panic(expected = "explorer service is already instantiated")]
fn cannot_initialize_blockchain_with_2_explorers() {
    let other_explorer = ExplorerFactory
        .artifact_id()
        .into_default_instance(100, "other-explorer");
    TestKitBuilder::validator()
        .with_default_rust_service(ExplorerFactory)
        .with_instance(other_explorer)
        .build();
}

#[test]
fn cannot_add_another_explorer() {
    let mut testkit = TestKitBuilder::validator()
        .with_default_rust_service(ExplorerFactory)
        .with_rust_service(Supervisor)
        .with_artifact(Supervisor.artifact_id())
        .with_instance(Supervisor::simple())
        .build();

    let deadline = Height(5);
    let config = ConfigPropose::new(0, deadline).start_service(
        ExplorerFactory.artifact_id(),
        "other_explorer",
        (),
    );
    let tx = testkit
        .us()
        .service_keypair()
        .propose_config_change(SUPERVISOR_ID, config);
    let block = testkit.create_block_with_transaction(tx);
    block[0].status().unwrap();

    testkit.create_blocks_until(deadline.previous());
    let block = testkit.create_block();
    // The service instantiation should have failed.
    let err = block.error_map()[&CallInBlock::after_transactions(SUPERVISOR_ID)];
    let expected_err = ErrorMatch::from_fail(&Error::DuplicateExplorer)
        .with_description_containing("explorer service is already instantiated")
        .in_call(CallType::Constructor);
    assert_eq!(*err, expected_err);
}
