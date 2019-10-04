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

//! The module responsible for the correct Exonum blockchain creation.

use futures::sync::mpsc;

use std::sync::Arc;

use crate::{
    blockchain::{Blockchain, ConsensusConfig, Schema},
    crypto::{PublicKey, SecretKey},
    events::InternalRequest,
    merkledb::{BinaryValue, Database},
    node::ApiSender,
    runtime::{
        dispatcher::Dispatcher,
        rust::{RustRuntime, ServiceFactory},
        InstanceId, InstanceSpec, Runtime,
    },
};

/// The object responsible for the correct Exonum blockchain creation from the components.
///
/// During the `Blockchain` creation it creates and commits a genesis block if the database
/// is empty. Otherwise, it restores the state from the database.
#[derive(Debug)]
pub struct BlockchainBuilder {
    /// The database which works under the hood.
    pub database: Arc<dyn Database>,
    /// Blockchain configuration used to create the genesis block.
    pub genesis_config: ConsensusConfig,
    /// Keypair, which  is used to sign service transactions on behalf of this node.
    pub service_keypair: (PublicKey, SecretKey),
    /// List of the supported runtimes.
    pub runtimes: Vec<(u32, Box<dyn Runtime>)>,
    /// List of the privileged services with the configuration parameters that are created directly
    /// in the genesis block.
    pub builtin_instances: Vec<InstanceConfig>,
}

impl BlockchainBuilder {
    /// Creates a new builder instance for the specified database, the genesis configuration and
    /// the service keypair without any runtimes. The user must add them by himself/herself.
    pub fn new(
        database: impl Into<Arc<dyn Database>>,
        genesis_config: ConsensusConfig,
        service_keypair: (PublicKey, SecretKey),
    ) -> Self {
        Self {
            database: database.into(),
            genesis_config,
            service_keypair,
            runtimes: Vec::new(),
            builtin_instances: Vec::new(),
        }
    }

    /// Add the built-in Rust runtime with the default built-in services.
    pub fn with_default_runtime(
        self,
        services: impl IntoIterator<Item = InstanceCollection>,
    ) -> Self {
        self.with_rust_runtime(services)
    }

    /// Add the built-in Rust runtime with the specified built-in services.
    pub fn with_rust_runtime(
        mut self,
        services: impl IntoIterator<Item = InstanceCollection>,
    ) -> Self {
        let mut runtime = RustRuntime::new();
        for service in services {
            runtime.add_service_factory(service.factory);
            self.builtin_instances.extend(service.instances);
        }
        self.with_additional_runtime(runtime)
    }

    pub fn with_external_runtimes(
        mut self,
        runtimes: impl IntoIterator<Item = impl Into<(u32, Box<dyn Runtime>)>>,
    ) -> Self {
        for runtime in runtimes {
            self.runtimes.push(runtime.into());
        }

        self
    }

    /// Add an additional runtime with the specified identifier.
    pub fn with_additional_runtime(mut self, runtime: impl Into<(u32, Box<dyn Runtime>)>) -> Self {
        self.runtimes.push(runtime.into());
        self
    }

    /// Add instance specifications of builtin services.
    pub fn with_builtin_instances(
        mut self,
        instances: impl IntoIterator<Item = InstanceConfig>,
    ) -> Self {
        self.builtin_instances.extend(instances);
        self
    }

    /// Returns blockchain instance, creates and commits the genesis block with the specified
    /// genesis configuration if the blockchain has not been initialized.
    /// Otherwise restores dispatcher state from database.
    ///
    /// # Panics
    ///
    /// * If the genesis block was not committed.
    /// * If storage version is not specified or not supported.
    pub fn finalize(
        self,
        api_sender: ApiSender,
        internal_requests: mpsc::Sender<InternalRequest>,
    ) -> Result<Blockchain, failure::Error> {
        let mut dispatcher = Dispatcher::with_runtimes(self.runtimes);
        // If genesis block had been already created just restores dispatcher state from database
        // otherwise creates genesis block with the given specification.
        let has_genesis_block = {
            let snapshot = self.database.snapshot();
            !Schema::new(snapshot.as_ref())
                .block_hashes_by_height()
                .is_empty()
        };

        let blockchain = if has_genesis_block {
            let snapshot = self.database.snapshot();
            dispatcher.restore_state(&snapshot)?;
            Blockchain::with_dispatcher(
                self.database,
                dispatcher,
                self.service_keypair,
                api_sender,
                internal_requests,
            )
        } else {
            // Creates blockchain with the new genesis block.
            let mut blockchain = Blockchain::with_dispatcher(
                self.database,
                dispatcher,
                self.service_keypair,
                api_sender,
                internal_requests,
            );
            // Adds builtin services.
            blockchain.merge({
                let mut fork = blockchain.fork();
                let mut dispatcher = blockchain.dispatcher();
                for instance_config in self.builtin_instances {
                    dispatcher.add_builtin_service(
                        &mut fork,
                        instance_config.instance_spec,
                        instance_config.artifact_spec.unwrap_or_default(),
                        instance_config.constructor,
                    )?;
                }
                fork.into_patch()
            })?;
            // Commits genesis block.
            blockchain.create_genesis_block(self.genesis_config)?;
            blockchain
        };
        // Starts built-in APIs.
        blockchain.notify_api_changes();
        Ok(blockchain)
    }
}

/// Instantiation parameters of service instance.
#[derive(Debug)]
pub struct InstanceConfig {
    /// Service instance specification.
    pub instance_spec: InstanceSpec,
    /// Artifact deploy specification.
    pub artifact_spec: Option<Vec<u8>>,
    /// Service configuration parameters.
    pub constructor: Vec<u8>,
}

impl InstanceConfig {
    pub fn new(
        instance_spec: InstanceSpec,
        artifact_spec: Option<Vec<u8>>,
        constructor: Vec<u8>,
    ) -> Self {
        Self {
            instance_spec,
            artifact_spec,
            constructor,
        }
    }
}

/// Rust runtime artifact with the list of instances.
#[derive(Debug)]
pub struct InstanceCollection {
    /// Rust services factory as a special case of an artifact.
    pub factory: Box<dyn ServiceFactory>,
    /// List of service instances with the initial configuration parameters.
    pub instances: Vec<InstanceConfig>,
}

impl InstanceCollection {
    /// Creates a new blank collection of instances for the specified service factory.
    pub fn new(factory: impl Into<Box<dyn ServiceFactory>>) -> Self {
        Self {
            factory: factory.into(),
            instances: Vec::new(),
        }
    }

    /// Add a new service instance to the collection.
    pub fn with_instance(
        mut self,
        id: InstanceId,
        name: impl Into<String>,
        params: impl BinaryValue,
    ) -> Self {
        let spec = InstanceSpec {
            artifact: self.factory.artifact_id().into(),
            id,
            name: name.into(),
        };
        let instance_config = InstanceConfig::new(spec, None, params.into_bytes());
        self.instances.push(instance_config);
        self
    }
}

#[cfg(test)]
mod tests {
    use exonum_merkledb::TemporaryDB;

    use crate::{
        crypto,
        helpers::{generate_testnet_config, Height},
    };

    // Import service from tests, so we won't have implement other one.
    use crate::blockchain::tests::ServiceGoodImpl as SampleService;

    use super::*;

    #[test]
    fn finalize_without_genesis_block() {
        let config = generate_testnet_config(1, 0)[0].clone();
        let service_keypair = config.service_keypair();

        let external_runtimes: Vec<(u32, Box<dyn Runtime>)> = vec![];
        let services = vec![];

        let blockchain = Blockchain::new(
            TemporaryDB::new(),
            external_runtimes,
            services,
            config.consensus,
            service_keypair,
            ApiSender::new(mpsc::channel(0).0),
            mpsc::channel(0).0,
        );

        let access = blockchain.snapshot();
        assert_eq!(Schema::new(access.as_ref()).height(), Height(0));
        // TODO check dispatcher schema.
    }

    #[test]
    #[should_panic(expected = "Specified service identifier is already used")]
    fn finalize_duplicate_services() {
        let config = generate_testnet_config(1, 0)[0].clone();
        let service_keypair = config.service_keypair();

        let external_runtimes: Vec<(u32, Box<dyn Runtime>)> = vec![];
        let services = vec![
            InstanceCollection::new(SampleService).with_instance(0, "sample", ()),
            InstanceCollection::new(SampleService).with_instance(0, "sample", ()),
        ];

        Blockchain::new(
            TemporaryDB::new(),
            external_runtimes,
            services,
            config.consensus,
            service_keypair,
            ApiSender::new(mpsc::channel(0).0),
            mpsc::channel(0).0,
        );
    }

    #[test]
    #[should_panic(expected = "Consensus configuration must have at least one validator")]
    fn finalize_invalid_consensus_config() {
        let consensus = ConsensusConfig::default();
        let service_keypair = crypto::gen_keypair();

        let external_runtimes: Vec<(u32, Box<dyn Runtime>)> = vec![];
        let services = vec![];

        Blockchain::new(
            TemporaryDB::new(),
            external_runtimes,
            services,
            consensus,
            service_keypair,
            ApiSender::new(mpsc::channel(0).0),
            mpsc::channel(0).0,
        );
    }
}
