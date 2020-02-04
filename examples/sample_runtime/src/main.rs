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

//! Example of a very simple runtime that can perform two types of transaction:
//! increment and reset counter in the service instance.

use exonum::{
    blockchain::{config::GenesisConfigBuilder, ConsensusConfig, ValidatorKeys},
    helpers::Height,
    keys::Keys,
    merkledb::{BinaryValue, Snapshot, TemporaryDB},
    runtime::{
        migrations::{InitMigrationError, MigrationScript},
        versioning::Version,
        AnyTx, ArtifactId, CallInfo, CommonError, ExecutionContext, ExecutionError, ExecutionFail,
        InstanceDescriptor, InstanceId, InstanceSpec, InstanceStatus, Mailbox, MethodId, Runtime,
        SnapshotExt, WellKnownRuntime, SUPERVISOR_INSTANCE_ID,
    },
};
use exonum_derive::*;
use exonum_node::{NodeApiConfig, NodeBuilder, NodeConfig};
use exonum_rust_runtime::{RustRuntime, ServiceFactory};
use exonum_supervisor::{ConfigPropose, DeployRequest, Supervisor, SupervisorInterface};
use futures::{Future, IntoFuture};

use std::{cell::Cell, collections::BTreeMap, thread, time::Duration};

/// Service instance with a counter.
#[derive(Debug, Default)]
struct SampleService {
    counter: Cell<u64>,
    name: String,
}

/// Sample runtime.
#[derive(Debug, Default)]
struct SampleRuntime {
    deployed_artifacts: BTreeMap<ArtifactId, Vec<u8>>,
    started_services: BTreeMap<InstanceId, SampleService>,
}

// Define runtime specific errors.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[derive(ExecutionFail)]
#[execution_fail(kind = "runtime")]
enum SampleRuntimeError {
    /// Incorrect information to call transaction.
    IncorrectCallInfo = 1,
    /// Incorrect transaction payload.
    IncorrectPayload = 2,
}

impl SampleRuntime {
    /// Create a new service instance with the given specification.
    fn start_service(
        &self,
        artifact: &ArtifactId,
        instance: &InstanceDescriptor,
    ) -> Result<SampleService, ExecutionError> {
        // Invariants guaranteed by the core.
        assert!(self.deployed_artifacts.contains_key(artifact));
        assert!(!self.started_services.contains_key(&instance.id));

        Ok(SampleService {
            name: instance.name.to_owned(),
            ..SampleService::default()
        })
    }

    /// In the present simplest case, the artifact is added into the deployed artifacts table.
    fn deploy_artifact(
        &mut self,
        artifact: ArtifactId,
        spec: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        // Invariant guaranteed by the core
        assert!(!self.deployed_artifacts.contains_key(&artifact));

        println!("Deploying artifact: {}", &artifact);
        self.deployed_artifacts.insert(artifact, spec);

        Ok(())
    }
}

impl Runtime for SampleRuntime {
    fn deploy_artifact(
        &mut self,
        artifact: ArtifactId,
        spec: Vec<u8>,
    ) -> Box<dyn Future<Item = (), Error = ExecutionError>> {
        Box::new(self.deploy_artifact(artifact, spec).into_future())
    }

    fn is_artifact_deployed(&self, id: &ArtifactId) -> bool {
        self.deployed_artifacts.contains_key(id)
    }

    /// Initiates adding a new service and sets the counter value for this.
    fn initiate_adding_service(
        &self,
        context: ExecutionContext<'_>,
        artifact: &ArtifactId,
        params: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        let service_instance = self.start_service(artifact, context.instance())?;
        let new_value = u64::from_bytes(params.into()).map_err(CommonError::malformed_arguments)?;
        service_instance.counter.set(new_value);
        println!(
            "Initializing service {}: {} with value {}",
            artifact,
            context.instance(),
            new_value
        );
        Ok(())
    }

    fn initiate_resuming_service(
        &self,
        _context: ExecutionContext<'_>,
        _artifact: &ArtifactId,
        _parameters: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        unreachable!("We don't resume services in this example.")
    }

    /// Commits status for the `SampleService` instance with the specified ID.
    fn update_service_status(
        &mut self,
        _snapshot: &dyn Snapshot,
        spec: &InstanceSpec,
        status: &InstanceStatus,
    ) {
        match status {
            InstanceStatus::Active => {
                // Unwrap here is safe, since by invocation of this method
                // `exonum` guarantees that `initiate_adding_service` was invoked
                // before and it returned `Ok(..)`.
                let instance = self
                    .start_service(&spec.artifact, &spec.as_descriptor())
                    .unwrap();
                println!("Starting service {}: {:?}", spec, instance);
                self.started_services.insert(spec.id, instance);
            }

            InstanceStatus::Stopped => {
                let instance = self.started_services.remove(&spec.id);
                println!("Stopping service {}: {:?}", spec, instance);
            }

            InstanceStatus::Migrating(_) => {
                // We don't migrate service data in this demo.
            }

            _ => {
                // We aren't interested in other possible statuses.
            }
        }
    }

    fn migrate(
        &self,
        _new_artifact: &ArtifactId,
        _data_version: &Version,
    ) -> Result<Option<MigrationScript>, InitMigrationError> {
        Err(InitMigrationError::NotSupported)
    }

    fn execute(
        &self,
        context: ExecutionContext<'_>,
        method_id: MethodId,
        payload: &[u8],
    ) -> Result<(), ExecutionError> {
        let service = self
            .started_services
            .get(&context.instance().id)
            .ok_or(SampleRuntimeError::IncorrectCallInfo)?;

        println!(
            "Executing method {}#{} of service {}",
            context.interface_name(),
            method_id,
            context.instance().id
        );

        const SERVICE_INTERFACE: &str = "";
        match (context.interface_name(), method_id) {
            // Increment counter.
            (SERVICE_INTERFACE, 0) => {
                let value = u64::from_bytes(payload.into())
                    .map_err(|e| SampleRuntimeError::IncorrectPayload.with_description(e))?;
                let counter = service.counter.get();
                println!("Updating counter value to {}", counter + value);
                service.counter.set(value + counter);
                Ok(())
            }

            // Reset counter.
            (SERVICE_INTERFACE, 1) => {
                if !payload.is_empty() {
                    Err(SampleRuntimeError::IncorrectPayload.into())
                } else {
                    println!("Resetting counter");
                    service.counter.set(0);
                    Ok(())
                }
            }

            // Unknown transaction.
            (interface, method) => {
                let err = SampleRuntimeError::IncorrectCallInfo.with_description(format!(
                    "Incorrect information to call transaction. {}#{}",
                    interface, method
                ));
                Err(err)
            }
        }
    }

    fn before_transactions(&self, _context: ExecutionContext<'_>) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn after_transactions(&self, _context: ExecutionContext<'_>) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn after_commit(&mut self, _snapshot: &dyn Snapshot, _mailbox: &mut Mailbox) {}
}

impl From<SampleRuntime> for (u32, Box<dyn Runtime>) {
    fn from(inner: SampleRuntime) -> Self {
        (SampleRuntime::ID, Box::new(inner))
    }
}

impl WellKnownRuntime for SampleRuntime {
    const ID: u32 = 255;
}

fn node_config() -> (NodeConfig, Keys) {
    let keys = Keys::random();
    let validator_keys = vec![ValidatorKeys::new(keys.consensus_pk(), keys.service_pk())];
    let consensus = ConsensusConfig::default().with_validator_keys(validator_keys);

    let api_address = "0.0.0.0:8000".parse().unwrap();
    let api_cfg = NodeApiConfig {
        public_api_address: Some(api_address),
        ..Default::default()
    };

    let peer_address = "0.0.0.0:2000";

    let node_config = NodeConfig {
        listen_address: peer_address.parse().unwrap(),
        consensus,
        external_address: peer_address.to_owned(),
        network: Default::default(),
        connect_list: Default::default(),
        api: api_cfg,
        mempool: Default::default(),
        thread_pool_size: Default::default(),
    };
    (node_config, keys)
}

fn main() {
    exonum::helpers::init_logger().unwrap();

    println!("Creating database in temporary dir...");

    let db = TemporaryDB::new();
    let (node_cfg, node_keys) = node_config();
    let consensus_config = node_cfg.consensus.clone();
    let service_keypair = node_keys.service.clone();
    let genesis_config = GenesisConfigBuilder::with_consensus_config(consensus_config)
        .with_artifact(Supervisor.artifact_id())
        .with_instance(Supervisor::simple())
        .build();

    println!("Creating blockchain with additional runtime...");
    let node = NodeBuilder::new(db, node_cfg, node_keys)
        .with_genesis_config(genesis_config)
        .with_runtime(SampleRuntime::default())
        .with_runtime_fn(|channel| {
            RustRuntime::builder()
                .with_factory(Supervisor)
                .build(channel.endpoints_sender())
        })
        .build();

    let blockchain_ref = node.blockchain().to_owned();
    let shutdown_handle = node.shutdown_handle();
    println!("Starting a single node...");
    println!("Blockchain is ready for transactions!");

    let handle = thread::spawn(move || {
        let deploy_height = Height(50);
        // Send an artifact `DeployRequest` to the sample runtime.
        let request = DeployRequest {
            artifact: "255:sample_artifact:0.1.0".parse().unwrap(),
            deadline_height: deploy_height,
            spec: Vec::default(),
        };
        let tx = service_keypair.request_artifact_deploy(SUPERVISOR_INSTANCE_ID, request);
        blockchain_ref
            .sender()
            .broadcast_transaction(tx)
            .wait()
            .unwrap();

        // Wait until the request is finished.
        thread::sleep(Duration::from_secs(5));

        // Send a `StartService` request to the sample runtime.
        let instance_name = "instance";
        let proposal = ConfigPropose::immediate(0).start_service(
            "255:sample_artifact:0.1.0".parse().unwrap(),
            instance_name,
            10_u64,
        );
        let proposal = service_keypair.propose_config_change(SUPERVISOR_INSTANCE_ID, proposal);
        blockchain_ref
            .sender()
            .broadcast_transaction(proposal)
            .wait()
            .unwrap();

        // Wait until instance identifier is assigned.
        thread::sleep(Duration::from_secs(1));

        // Get an instance identifier.
        let snapshot = blockchain_ref.snapshot();
        let state = snapshot
            .for_dispatcher()
            .get_instance(instance_name)
            .unwrap();
        assert_eq!(state.status.unwrap(), InstanceStatus::Active);
        let instance_id = state.spec.id;

        // Send an update counter transaction.
        let tx = AnyTx::new(CallInfo::new(instance_id, 0), 1_000_u64.into_bytes());
        let tx = tx.sign_with_keypair(&service_keypair);
        blockchain_ref
            .sender()
            .broadcast_transaction(tx)
            .wait()
            .unwrap();
        thread::sleep(Duration::from_secs(2));

        // Send a reset counter transaction.
        let tx = AnyTx::new(CallInfo::new(instance_id, 1), vec![]);
        let tx = tx.sign_with_keypair(&service_keypair);
        blockchain_ref
            .sender()
            .broadcast_transaction(tx)
            .wait()
            .unwrap();

        thread::sleep(Duration::from_secs(2));
        shutdown_handle.shutdown().wait().unwrap();
    });

    node.run().unwrap();
    handle.join().unwrap();
}
