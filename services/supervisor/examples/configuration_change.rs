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

use exonum::{
    helpers::Height,
    runtime::{ExecutionContext, ExecutionError, InstanceId, SnapshotExt, SUPERVISOR_INSTANCE_ID},
};
use exonum_derive::*;
use exonum_merkledb::{
    access::{Access, AccessExt, FromAccess},
    Entry, ObjectHash,
};
use exonum_rust_runtime::{Service, ServiceFactory};
use exonum_testkit::{TestKit, TestKitBuilder};

use exonum_supervisor::{ConfigPropose, ConfigVote, Configure, Supervisor, SupervisorInterface};

const SERVICE_ID: InstanceId = 256;
const SERVICE_NAME: &str = "config";

// Simple service to provide an example of how to implement the service configuration change.
#[derive(Clone, Default, Debug, ServiceFactory, ServiceDispatcher)]
#[service_dispatcher(implements(raw = "Configure<Params = String>"))]
#[service_factory(artifact_name = "config", artifact_version = "1.0.0")]
pub struct ConfigChangeService;

#[derive(Debug, FromAccess)]
pub struct Schema<T: Access> {
    params: Entry<T::Base, String>,
}

impl<T: Access> Schema<T> {
    fn new(access: T) -> Self {
        Self::from_root(access).unwrap()
    }
}

impl Service for ConfigChangeService {}

// To allow service change its configuration we need to implement `Configure` trait.
impl Configure for ConfigChangeService {
    type Params = String;

    fn verify_config(
        &self,
        _context: ExecutionContext<'_>,
        params: Self::Params,
    ) -> Result<(), ExecutionError> {
        println!("Verify config called with params {}", params);
        Ok(())
    }

    fn apply_config(
        &self,
        context: ExecutionContext<'_>,
        params: Self::Params,
    ) -> Result<(), ExecutionError> {
        println!("Apply config called with params {}", params);
        Schema::new(context.service_data())
            .params
            .set(params.clone());

        Ok(())
    }
}

fn main() {
    let service = ConfigChangeService;
    let artifact = service.artifact_id();

    // Create testkit instance with our test service and supervisor.
    let mut testkit = TestKitBuilder::validator()
        .with_logger()
        .with_validators(4)
        .with_rust_service(Supervisor)
        .with_artifact(Supervisor.artifact_id())
        .with_instance(Supervisor::decentralized())
        .with_artifact(artifact.clone())
        .with_instance(artifact.into_default_instance(SERVICE_ID, SERVICE_NAME))
        .with_rust_service(service)
        .build();

    // Firstly, lets change consensus configuration and increase `min_propose_timeout`.
    let cfg_change_height = Height(5);

    // Get current configuration and update `min_propose_timeout`.
    let mut cfg = testkit.consensus_config();
    cfg.min_propose_timeout += 1;

    // Create consensus config propose.
    let propose = ConfigPropose::new(0, cfg_change_height).consensus_config(cfg.clone());

    // Sign propose and send it to validators, then wait for its confirmation.
    send_and_vote_for_propose(&mut testkit, propose, cfg_change_height);

    // Check that the proposal has become actual.
    assert_eq!(testkit.consensus_config(), cfg);

    // Secondly, lets change our service configuration. Since config represents as string, lets
    // provide new string value.
    let params = "new value".to_owned();

    let cfg_change_height = Height(10);
    let propose = ConfigPropose::new(1, cfg_change_height).service_config(256, params.clone());

    send_and_vote_for_propose(&mut testkit, propose, cfg_change_height);

    // Check that parameter has been changed.
    let snapshot = testkit.snapshot();
    let actual_params = snapshot
        .for_service(SERVICE_NAME)
        .unwrap()
        .get_entry::<_, String>("params");

    assert_eq!(actual_params.get().unwrap(), params);
}

fn send_and_vote_for_propose(
    testkit: &mut TestKit,
    propose: ConfigPropose,
    cfg_change_height: Height,
) {
    let initiator_id = testkit.network().us().validator_id().unwrap();
    let keys = testkit.validator(initiator_id).service_keypair();
    let propose_hash = propose.object_hash();

    // Sign propose with validator keys.
    let signed_proposal = keys.propose_config_change(SUPERVISOR_INSTANCE_ID, propose);

    // Create block with this proposal.
    testkit
        .create_block_with_transaction(signed_proposal)
        .transactions[0]
        .status()
        .unwrap();

    // Create signed transactions for all validators.
    let signed_txs: Vec<_> = testkit
        .network()
        .validators()
        .iter()
        .filter(|validator| validator.validator_id() != Some(initiator_id))
        .map(|validator| {
            validator
                .service_keypair()
                .confirm_config_change(SUPERVISOR_INSTANCE_ID, ConfigVote { propose_hash })
        })
        .collect();

    // Confirm this propose.
    testkit
        .create_block_with_transactions(signed_txs)
        .transactions[0]
        .status()
        .unwrap();

    testkit.create_blocks_until(cfg_change_height);
}
