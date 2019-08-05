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
    blockchain::{BlockchainBuilder, GenesisConfig, ValidatorKeys},
    crypto::{PublicKey, SecretKey},
    helpers::Height,
    messages::Verified,
    node::{ApiSender, Node, NodeApiConfig, NodeChannel, NodeConfig},
    proto::Any,
    runtime::{
        dispatcher::{self, Dispatcher, DispatcherSender, Error as DispatcherError},
        rust::Transaction,
        supervisor::{DeployRequest, StartService, Supervisor},
        AnyTx, ArtifactId, ArtifactInfo, CallInfo, ExecutionContext, ExecutionError, InstanceSpec,
        Runtime, ServiceInstanceId, StateHashAggregator,
    },
};
use exonum_derive::IntoExecutionError;
use exonum_merkledb::{BinaryValue, Fork, Snapshot, TemporaryDB};
use futures::{Future, IntoFuture};

use std::{
    cell::Cell,
    collections::btree_map::{BTreeMap, Entry},
    convert::TryFrom,
    thread,
    time::Duration,
};

/// Service instance with a counter.
#[derive(Debug, Default)]
struct SampleService {
    counter: Cell<u64>,
}

/// Sample runtime.
#[derive(Debug, Default)]
struct SampleRuntime {
    deployed_artifacts: BTreeMap<ArtifactId, Any>,
    started_services: BTreeMap<ServiceInstanceId, SampleService>,
}

// Define runtime specific errors.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, IntoExecutionError)]
#[exonum(kind = "runtime")]
enum SampleRuntimeError {
    /// Unable to parse service configuration.
    ConfigParseError = 0,
    /// Incorrect information to call transaction.
    IncorrectCallInfo = 1,
    /// Incorrect transaction payload.
    IncorrectPayload = 2,
}

impl SampleRuntime {
    /// Runtime identifier for this runtime.
    const ID: u32 = 255;
}

impl Runtime for SampleRuntime {
    /// In the present simplest case, the artifact is added into the deployed artifacts table.
    fn deploy_artifact(
        &mut self,
        artifact: ArtifactId,
        spec: Any,
    ) -> Box<dyn Future<Item = (), Error = ExecutionError>> {
        Box::new(
            match self.deployed_artifacts.entry(artifact) {
                Entry::Occupied(_) => Err(DispatcherError::ArtifactAlreadyDeployed),
                Entry::Vacant(entry) => {
                    println!("Deploying artifact: {}", entry.key());
                    entry.insert(spec);
                    Ok(())
                }
            }
            .map_err(ExecutionError::from)
            .into_future(),
        )
    }

    /// `start_service` request creates a new `SampleService` instance with the specified ID.
    fn start_service(&mut self, spec: &InstanceSpec) -> Result<(), ExecutionError> {
        if !self.deployed_artifacts.contains_key(&spec.artifact) {
            return Err(DispatcherError::ArtifactNotDeployed.into());
        }
        if self.started_services.contains_key(&spec.id) {
            return Err(DispatcherError::ServiceIdExists.into());
        }

        self.started_services
            .insert(spec.id, SampleService::default());
        println!("Starting service: {:?}", spec);
        Ok(())
    }

    /// `configure_service` request sets the counter value of the corresponding
    /// `SampleService` instance
    fn configure_service(
        &self,
        _context: &Fork,
        spec: &InstanceSpec,
        parameters: Any,
    ) -> Result<(), ExecutionError> {
        let service_instance = self
            .started_services
            .get(&spec.id)
            .ok_or(DispatcherError::ServiceNotStarted)?;

        let new_value =
            u64::try_from(parameters).map_err(|e| (SampleRuntimeError::ConfigParseError, e))?;
        service_instance.counter.set(new_value);
        println!("Configuring service {} with value {}", spec.name, new_value);
        Ok(())
    }

    /// `stop_service` removes the service with the specified ID from the list of the started services.
    fn stop_service(&mut self, spec: &InstanceSpec) -> Result<(), ExecutionError> {
        println!("Stopping service: {:?}", spec);
        self.started_services
            .remove(&spec.id)
            .map(drop)
            .ok_or(DispatcherError::ServiceNotStarted)
            .map_err(ExecutionError::from)
    }

    fn execute(
        &self,
        _dispatcher: &Dispatcher,
        _context: &mut ExecutionContext,
        call_info: CallInfo,
        payload: &[u8],
    ) -> Result<(), ExecutionError> {
        let service = self
            .started_services
            .get(&call_info.instance_id)
            .ok_or(SampleRuntimeError::IncorrectCallInfo)?;

        println!(
            "Executing method {} of service {}",
            call_info.method_id, call_info.instance_id
        );

        // Very simple transaction executor.
        match call_info.method_id {
            // Increment counter.
            0 => {
                let value = u64::from_bytes(payload.into())
                    .map_err(|e| (SampleRuntimeError::IncorrectPayload, e))?;
                let counter = service.counter.get();
                println!("Updating counter value to {}", counter + value);
                service.counter.set(value + counter);
                Ok(())
            }

            // Reset counter.
            1 => {
                if !payload.is_empty() {
                    Err(SampleRuntimeError::IncorrectPayload.into())
                } else {
                    println!("Resetting counter");
                    service.counter.set(0);
                    Ok(())
                }
            }
            // Unknown transaction.
            _ => Err(SampleRuntimeError::IncorrectCallInfo.into()),
        }
    }

    fn artifact_info(&self, id: &ArtifactId) -> Option<ArtifactInfo> {
        self.deployed_artifacts
            .get(id)
            .map(|_| ArtifactInfo::default())
    }

    fn state_hashes(&self, _snapshot: &dyn Snapshot) -> StateHashAggregator {
        StateHashAggregator::default()
    }

    fn before_commit(&self, _dispatcher: &Dispatcher, _fork: &mut Fork) {}

    fn after_commit(
        &self,
        _dispatcher: &DispatcherSender,
        _snapshot: &dyn Snapshot,
        _service_keypair: &(PublicKey, SecretKey),
        _tx_sender: &ApiSender,
    ) {
    }
}

impl From<SampleRuntime> for (u32, Box<dyn Runtime>) {
    fn from(inner: SampleRuntime) -> Self {
        (SampleRuntime::ID, Box::new(inner))
    }
}

fn node_config() -> NodeConfig {
    let (consensus_public_key, consensus_secret_key) = exonum::crypto::gen_keypair();
    let (service_public_key, service_secret_key) = exonum::crypto::gen_keypair();

    let validator_keys = ValidatorKeys {
        consensus_key: consensus_public_key,
        service_key: service_public_key,
    };
    let genesis = GenesisConfig::new(vec![validator_keys].into_iter());

    let api_address = "0.0.0.0:8000".parse().unwrap();
    let api_cfg = NodeApiConfig {
        public_api_address: Some(api_address),
        ..Default::default()
    };

    let peer_address = "0.0.0.0:2000";

    NodeConfig {
        listen_address: peer_address.parse().unwrap(),
        service_public_key,
        service_secret_key,
        consensus_public_key,
        consensus_secret_key,
        genesis,
        external_address: peer_address.to_owned(),
        network: Default::default(),
        connect_list: Default::default(),
        api: api_cfg,
        mempool: Default::default(),
        services_configs: Default::default(),
        database: Default::default(),
        thread_pool_size: Default::default(),
    }
}

fn main() {
    exonum::helpers::init_logger().unwrap();

    println!("Creating database in temporary dir...");

    let db = TemporaryDB::new();
    let node_cfg = node_config();
    let genesis = node_cfg.genesis.clone();
    let service_keypair = node_cfg.service_keypair();
    let channel = NodeChannel::new(&node_cfg.mempool.events_pool_capacity);
    let api_sender = ApiSender::new(channel.api_requests.0.clone());

    println!("Creating blockchain with additional runtime...");
    // Create blockchain with rust runtime and our additional runtime.
    let blockchain = BlockchainBuilder::new(db, genesis, service_keypair.clone())
        .with_default_runtime(vec![])
        .with_additional_runtime(SampleRuntime::default())
        .finalize(api_sender.clone(), channel.internal_requests.0.clone())
        .unwrap();

    let node = Node::with_blockchain(blockchain.clone(), channel, node_cfg, None);
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
                    spec: Any::default(),
                }
                .sign(
                    Supervisor::BUILTIN_ID,
                    service_keypair.0,
                    &service_keypair.1,
                ),
            )
            .unwrap();
        // Wait until the request is finished.
        thread::sleep(Duration::from_secs(5));

        // Sends start service request to the sample runtime.
        let instance_name = "instance".to_owned();
        api_sender
            .broadcast_transaction(
                StartService {
                    artifact: "255:sample_artifact".parse().unwrap(),
                    name: instance_name.clone(),
                    config: Any::from(10_u64),
                    deadline_height,
                }
                .sign(
                    Supervisor::BUILTIN_ID,
                    service_keypair.0,
                    &service_keypair.1,
                ),
            )
            .unwrap();
        // Wait until the request is finished.
        thread::sleep(Duration::from_secs(5));

        // Gets assigned instance identifier.
        let snapshot = blockchain.snapshot();
        let instance_id = dispatcher::Schema::new(snapshot.as_ref())
            .service_instances()
            .get(&instance_name)
            .unwrap()
            .id;
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
    });

    node.run().unwrap();
    handle.join().unwrap();
}
