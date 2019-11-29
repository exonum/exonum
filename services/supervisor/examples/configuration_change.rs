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
    blockchain::InstanceCollection,
    helpers::Height,
    messages::Verified,
    runtime::{
        rust::{CallContext, Service, Transaction},
        AnyTx, BlockchainData, ExecutionError, InstanceId, SnapshotExt, SUPERVISOR_INSTANCE_ID,
    },
};
use exonum_crypto::Hash;
use exonum_derive::*;
use exonum_merkledb::{
    access::{Access, AccessExt},
    Entry, ObjectHash, Snapshot,
};
use exonum_supervisor::{ConfigPropose, ConfigVote, Configure, DecentralizedSupervisor};
use exonum_testkit::{TestKit, TestKitBuilder};

const SERVICE_ID: InstanceId = 256;
const SERVICE_NAME: &str = "config";

// Simple service to provide an example of how to implement the service configuration change.
#[derive(Clone, Default, Debug, ServiceFactory, ServiceDispatcher)]
#[service_dispatcher(implements("ConfigChangeInterface", "Configure<Params = String>"))]
#[service_factory(artifact_name = "config", artifact_version = "1.0.0")]
pub struct ConfigChangeService;

#[exonum_interface]
pub trait ConfigChangeInterface {}

#[derive(Debug, FromAccess)]
pub struct Schema<T: Access> {
    params: Entry<T::Base, String>,
}

impl Service for ConfigChangeService {
    fn state_hash(&self, _data: BlockchainData<'_, &dyn Snapshot>) -> Vec<Hash> {
        vec![]
    }
}

impl ConfigChangeInterface for ConfigChangeService {}

// To allow service change its configuration we need to implement `Configure` trait.
impl Configure for ConfigChangeService {
    type Params = String;

    fn verify_config(
        &self,
        _context: CallContext<'_>,
        params: Self::Params,
    ) -> Result<(), ExecutionError> {
        println!("Verify config called with params {}", params);
        Ok(())
    }

    fn apply_config(
        &self,
        context: CallContext<'_>,
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
    let collection = InstanceCollection::new(service).with_instance(SERVICE_ID, SERVICE_NAME, ());

    // Create testkit instance with our test service and supervisor.
    let mut testkit = TestKitBuilder::validator()
        .with_logger()
        .with_validators(4)
        .with_rust_service(DecentralizedSupervisor::new())
        .with_rust_service(collection)
        .create();

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
    let signed_proposal = propose.sign_for_supervisor(keys.0, &keys.1);

    // Create block with this proposal.
    testkit
        .create_block_with_transaction(signed_proposal)
        .transactions[0]
        .status()
        .unwrap();

    // Create signed transactions for all validators.
    let signed_txs = testkit
        .network()
        .validators()
        .iter()
        .filter(|validator| validator.validator_id() != Some(initiator_id))
        .map(|validator| {
            let keys = validator.service_keypair();
            ConfigVote { propose_hash }.sign(SUPERVISOR_INSTANCE_ID, keys.0, &keys.1)
        })
        .collect::<Vec<Verified<AnyTx>>>();

    // Confirm this propose.
    testkit
        .create_block_with_transactions(signed_txs)
        .transactions[0]
        .status()
        .unwrap();

    testkit.create_blocks_until(cfg_change_height);
}
