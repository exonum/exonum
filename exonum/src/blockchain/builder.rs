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

use exonum_merkledb::Database;
use futures::sync::mpsc;

use std::sync::Arc;

use crate::{
    blockchain::{Blockchain, GenesisConfig, Schema},
    crypto::{PublicKey, SecretKey},
    events::InternalRequest,
    node::ApiSender,
    proto::Any,
    runtime::{
        dispatcher::Dispatcher,
        rust::{RustRuntime, ServiceFactory},
        supervisor::Supervisor,
        InstanceSpec, Runtime, ServiceInstanceId,
    },
};

/// The object responsible for the correct Exonum blockchain creation from components.
///
/// During the `Blockchain` creation it creates and commits genesis block if database
/// is empty, otherwise it just restores state from database.
#[derive(Debug)]
pub struct BlockchainBuilder {
    /// The database which works under the hood.
    pub database: Arc<dyn Database>,
    /// Blockchain configuration which uses to create genesis block.
    pub genesis_config: GenesisConfig,
    /// Keypair, which  is used to sign service transactions on behalf of this node.
    pub service_keypair: (PublicKey, SecretKey),
    /// List of supported runtimes.
    pub runtimes: Vec<(u32, Box<dyn Runtime>)>,
    /// List of privileged services with configuration parameters that are created directly
    /// in the genesis block.
    pub builtin_instances: Vec<(InstanceSpec, Any)>,
}

impl BlockchainBuilder {
    /// Creates a new builder instance for the specified database, genesis configuration and
    /// service keypair without any runtimes. The user must add them by himself.
    pub fn new(
        database: impl Into<Arc<dyn Database>>,
        genesis_config: GenesisConfig,
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

    /// Adds built-in Rust runtime with the default built-in services.
    ///
    /// # List of built-in services to be added:
    ///
    /// * The [`Supervisor`] service, which is responsible for adding, modifying and removing user
    /// services during the operation of the blockchain.
    ///
    /// [`Supervisor`]: ../runtime/supervisor/index.html
    pub fn with_default_runtime(
        self,
        services: impl IntoIterator<Item = InstanceCollection>,
    ) -> Self {
        // Add the built-in `Supervisor` service.
        let mut services = services.into_iter().collect::<Vec<_>>();
        services.push(InstanceCollection::new(Supervisor).with_instance(
            Supervisor::BUILTIN_ID,
            Supervisor::BUILTIN_NAME,
            (),
        ));
        self.with_rust_runtime(services)
    }

    /// Adds built-in Rust runtime with the specified built-in services.
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

    /// Adds additional runtime with the specified identifier.
    pub fn with_additional_runtime(mut self, runtime: impl Into<(u32, Box<dyn Runtime>)>) -> Self {
        self.runtimes.push(runtime.into());
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

        Ok(if has_genesis_block {
            let snapshot = self.database.snapshot();
            dispatcher.restore_state(snapshot.as_ref())?;
            Blockchain::with_dispatcher(
                self.database,
                dispatcher,
                self.service_keypair.0,
                self.service_keypair.1,
                api_sender,
                internal_requests,
            )
        } else {
            // Creates blockchain with the new genesis block.
            let mut blockchain = Blockchain::with_dispatcher(
                self.database,
                dispatcher,
                self.service_keypair.0,
                self.service_keypair.1,
                api_sender,
                internal_requests,
            );
            // Adds builtin services.
            blockchain.merge({
                let fork = blockchain.fork();
                let mut dispatcher = blockchain.dispatcher();
                for service in self.builtin_instances {
                    dispatcher.add_builtin_service(&fork, service.0, service.1)?;
                }
                fork.into_patch()
            })?;
            // Commits genesis block.
            blockchain.create_genesis_block(self.genesis_config)?;
            blockchain
        })
    }
}

/// Rust runtime artifact with the list of instances.
#[derive(Debug)]
pub struct InstanceCollection {
    /// Rust services factory as a special case of an artifact.
    pub factory: Box<dyn ServiceFactory>,
    /// List of service instances with the initial configuration parameters.
    pub instances: Vec<(InstanceSpec, Any)>,
}

impl InstanceCollection {
    /// Creates a new blank collection of instances for the specified service factory.
    pub fn new(factory: impl Into<Box<dyn ServiceFactory>>) -> Self {
        Self {
            factory: factory.into(),
            instances: Vec::new(),
        }
    }

    /// Adds a new service instance to the collection.
    pub fn with_instance(
        mut self,
        id: ServiceInstanceId,
        name: impl Into<String>,
        params: impl Into<Any>,
    ) -> Self {
        let spec = InstanceSpec {
            artifact: self.factory.artifact_id().into(),
            id,
            name: name.into(),
        };
        let constructor = params.into();
        self.instances.push((spec, constructor));
        self
    }
}

#[cfg(test)]
mod tests {
    use exonum_merkledb::TemporaryDB;

    use crate::{
        helpers::{generate_testnet_config, Height},
        runtime::supervisor::Supervisor,
    };

    use super::*;

    #[test]
    fn finalize_without_genesis_block() {
        let config = generate_testnet_config(1, 0)[0].clone();
        let service_keypair = config.service_keypair();

        let blockchain = Blockchain::new(
            TemporaryDB::new(),
            Vec::new(),
            config.genesis,
            service_keypair,
            ApiSender::new(mpsc::unbounded().0),
            mpsc::channel(0).0,
        );

        let access = blockchain.snapshot();
        assert_eq!(Schema::new(access.as_ref()).height(), Height(0));
        // TODO check dispatcher schema.
    }

    #[test]
    #[should_panic(expected = "ExecutionError { kind: Dispatcher { code: 2 }")]
    fn finalize_duplicate_services() {
        let config = generate_testnet_config(1, 0)[0].clone();
        let service_keypair = config.service_keypair();

        Blockchain::new(
            TemporaryDB::new(),
            vec![InstanceCollection::new(Supervisor).with_instance(
                Supervisor::BUILTIN_ID,
                Supervisor::BUILTIN_NAME,
                (),
            )],
            config.genesis,
            service_keypair,
            ApiSender::new(mpsc::unbounded().0),
            mpsc::channel(0).0,
        );
    }
}
