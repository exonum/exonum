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

//! Example of a very simple runtime that can perform two types of transaction:
//! increment and reset counter in the service instance.

use exonum::{
    blockchain::{
        Blockchain, BlockchainBuilder, ConsensusConfig, InstanceCollection, ValidatorKeys,
    },
    helpers::Height,
    keys::Keys,
    merkledb::{BinaryValue, Snapshot, TemporaryDB},
    messages::Verified,
    node::{ApiSender, ExternalMessage, Node, NodeApiConfig, NodeChannel, NodeConfig},
    runtime::{
        rust::Transaction, AnyTx, ArtifactId, CallInfo, DeployStatus, DispatcherError,
        DispatcherSchema, ExecutionContext, ExecutionError, InstanceId, InstanceSpec, Mailbox,
        Runtime, StateHashAggregator, SUPERVISOR_INSTANCE_ID,
    },
};
use exonum_derive::IntoExecutionError;
use exonum_supervisor::{decentralized_supervisor, DeployRequest, StartService};
use futures::{Future, IntoFuture};

use std::{
    cell::Cell,
    collections::btree_map::{BTreeMap, Entry},
    thread,
    time::Duration,
};

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
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, IntoExecutionError)]
#[exonum(kind = "runtime")]
enum SampleRuntimeError {
    /// Incorrect information to call transaction.
    IncorrectCallInfo = 1,
    /// Incorrect transaction payload.
    IncorrectPayload = 2,
}

impl SampleRuntime {
    /// Runtime identifier for the present runtime.
    const ID: u32 = 255;

    /// Create a new service instance with the given specification.
    fn start_service(&self, spec: &InstanceSpec) -> Result<SampleService, ExecutionError> {
        if !self.deployed_artifacts.contains_key(&spec.artifact) {
            return Err(DispatcherError::ArtifactNotDeployed.into());
        }
        if self.started_services.contains_key(&spec.id) {
            return Err(DispatcherError::ServiceIdExists.into());
        }

        Ok(SampleService {
            name: spec.name.clone(),
            ..SampleService::default()
        })
    }

    /// In the present simplest case, the artifact is added into the deployed artifacts table.
    fn deploy_artifact(
        &mut self,
        artifact: ArtifactId,
        spec: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        match self.deployed_artifacts.entry(artifact) {
            Entry::Occupied(_) => Err(DispatcherError::ArtifactAlreadyDeployed.into()),
            Entry::Vacant(entry) => {
                println!("Deploying artifact: {}", entry.key());
                entry.insert(spec);
                Ok(())
            }
        }
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

    /// Starts an existing `SampleService` instance with the specified ID.
    fn commit_service(
        &mut self,
        _snapshot: &dyn Snapshot,
        spec: &InstanceSpec,
    ) -> Result<(), ExecutionError> {
        let instance = self.start_service(spec)?;
        println!("Starting service {}: {:?}", spec, instance);
        self.started_services.insert(spec.id, instance);
        Ok(())
    }

    /// Starts a new service instance and sets the counter value for this.
    fn start_adding_service(
        &self,
        _context: ExecutionContext<'_>,
        spec: &InstanceSpec,
        params: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        let service_instance = self.start_service(spec)?;
        let new_value =
            u64::from_bytes(params.into()).map_err(DispatcherError::malformed_arguments)?;
        service_instance.counter.set(new_value);
        println!("Initializing service {} with value {}", spec, new_value);
        Ok(())
    }

    fn execute(
        &self,
        context: ExecutionContext<'_>,
        call_info: &CallInfo,
        payload: &[u8],
    ) -> Result<(), ExecutionError> {
        let service = self
            .started_services
            .get(&call_info.instance_id)
            .ok_or(SampleRuntimeError::IncorrectCallInfo)?;

        println!(
            "Executing method {}#{} of service {}",
            context.interface_name, call_info.method_id, call_info.instance_id
        );

        const SERVICE_INTERFACE: &str = "";
        match (context.interface_name, call_info.method_id) {
            // Increment counter.
            (SERVICE_INTERFACE, 0) => {
                let value = u64::from_bytes(payload.into())
                    .map_err(|e| (SampleRuntimeError::IncorrectPayload, e))?;
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
                let err = (
                    SampleRuntimeError::IncorrectCallInfo,
                    format!(
                        "Incorrect information to call transaction. {}#{}",
                        interface, method
                    ),
                );
                Err(err.into())
            }
        }
    }

    fn state_hashes(&self, _snapshot: &dyn Snapshot) -> StateHashAggregator {
        StateHashAggregator::default()
    }

    fn before_commit(
        &self,
        _context: ExecutionContext<'_>,
        _id: InstanceId,
    ) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn after_commit(&mut self, _snapshot: &dyn Snapshot, _mailbox: &mut Mailbox) {}
}

impl From<SampleRuntime> for (u32, Box<dyn Runtime>) {
    fn from(inner: SampleRuntime) -> Self {
        (SampleRuntime::ID, Box::new(inner))
    }
}

fn node_config() -> NodeConfig {
    let (consensus_public_key, consensus_secret_key) = exonum::crypto::gen_keypair();
    let (service_public_key, service_secret_key) = exonum::crypto::gen_keypair();

    let consensus = ConsensusConfig {
        validator_keys: vec![ValidatorKeys {
            consensus_key: consensus_public_key,
            service_key: service_public_key,
        }],
        ..ConsensusConfig::default()
    };

    let keys = Keys::from_keys(
        consensus_public_key,
        consensus_secret_key,
        service_public_key,
        service_secret_key,
    );

    let api_address = "0.0.0.0:8000".parse().unwrap();
    let api_cfg = NodeApiConfig {
        public_api_address: Some(api_address),
        ..Default::default()
    };

    let peer_address = "0.0.0.0:2000";

    NodeConfig {
        listen_address: peer_address.parse().unwrap(),
        consensus,
        external_address: peer_address.to_owned(),
        network: Default::default(),
        connect_list: Default::default(),
        api: api_cfg,
        mempool: Default::default(),
        services_configs: Default::default(),
        database: Default::default(),
        thread_pool_size: Default::default(),
        master_key_path: Default::default(),
        keys,
    }
}

fn main() {
    exonum::helpers::init_logger().unwrap();

    println!("Creating database in temporary dir...");

    let db = TemporaryDB::new();
    let node_cfg = node_config();
    let genesis = node_cfg.consensus.clone();
    let service_keypair = node_cfg.service_keypair();
    let channel = NodeChannel::new(&node_cfg.mempool.events_pool_capacity);
    let api_sender = ApiSender::new(channel.api_requests.0.clone());

    println!("Creating blockchain with additional runtime...");
    // Create a blockchain with the Rust runtime and our additional runtime.
    let blockchain_base = Blockchain::new(db, service_keypair.clone(), api_sender.clone());
    let blockchain = BlockchainBuilder::new(blockchain_base, genesis)
        .with_rust_runtime(
            channel.endpoints.0.clone(),
            vec![InstanceCollection::from(decentralized_supervisor())],
        )
        .with_additional_runtime(SampleRuntime::default())
        .build()
        .unwrap();

    let blockchain_ref = blockchain.as_ref().to_owned();
    let node = Node::with_blockchain(blockchain, channel, node_cfg, None);
    println!("Starting a single node...");
    println!("Blockchain is ready for transactions!");

    let handle = thread::spawn(move || {
        let deadline_height = Height(10_000_000);
        // Send an artifact `DeployRequest` to the sample runtime.
        api_sender
            .broadcast_transaction(
                DeployRequest {
                    artifact: "255:sample_artifact".parse().unwrap(),
                    deadline_height,
                    spec: Vec::default(),
                }
                .sign(
                    SUPERVISOR_INSTANCE_ID,
                    service_keypair.0,
                    &service_keypair.1,
                ),
            )
            .unwrap();
        // Wait until the request is finished.
        thread::sleep(Duration::from_secs(5));

        // Send a `StartService` request to the sample runtime.
        let instance_name = "instance".to_owned();
        api_sender
            .broadcast_transaction(
                StartService {
                    artifact: "255:sample_artifact".parse().unwrap(),
                    name: instance_name.clone(),
                    config: 10_u64.into_bytes(),
                    deadline_height,
                }
                .sign(
                    SUPERVISOR_INSTANCE_ID,
                    service_keypair.0,
                    &service_keypair.1,
                ),
            )
            .unwrap();
        // Wait until instance identifier is assigned.
        thread::sleep(Duration::from_secs(5));

        // Get an instance identifier.
        let snapshot = blockchain_ref.snapshot();
        let (spec, status) = DispatcherSchema::new(snapshot.as_ref())
            .get_instance(instance_name.as_str())
            .unwrap();
        assert_eq!(status, DeployStatus::Active);
        let instance_id = spec.id;
        // Send an update counter transaction.
        api_sender
            .broadcast_transaction(Verified::from_value(
                AnyTx {
                    call_info: CallInfo {
                        instance_id,
                        method_id: 0,
                    },
                    arguments: 1_000_u64.into_bytes(),
                },
                service_keypair.0,
                &service_keypair.1,
            ))
            .unwrap();
        thread::sleep(Duration::from_secs(2));
        // Send a reset counter transaction.
        api_sender
            .broadcast_transaction(Verified::from_value(
                AnyTx {
                    call_info: CallInfo {
                        instance_id,
                        method_id: 1,
                    },
                    arguments: Vec::default(),
                },
                service_keypair.0,
                &service_keypair.1,
            ))
            .unwrap();
        thread::sleep(Duration::from_secs(2));
        api_sender
            .send_external_message(ExternalMessage::Shutdown)
            .unwrap();
    });

    node.run().unwrap();
    handle.join().unwrap();
}
