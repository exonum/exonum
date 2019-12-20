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

use crate::{
    blockchain::{config::GenesisConfig, Blockchain, BlockchainMut, Schema},
    runtime::{Dispatcher, RuntimeInstance},
};

/// The object responsible for the correct Exonum blockchain creation from the components.
///
/// During the `Blockchain` creation it creates and commits a genesis block if the database
/// is empty. Otherwise, it restores the state from the database.
// TODO: refine interface [ECR-3744]
#[derive(Debug)]
pub struct BlockchainBuilder {
    /// Underlying shared blockchain instance.
    blockchain: Blockchain,
    /// List of the supported runtimes.
    runtimes: Vec<RuntimeInstance>,
    /// Blockchain configuration used to create the genesis block.
    genesis_config: GenesisConfig,
}

impl BlockchainBuilder {
    /// Creates a new builder instance based on the `Blockchain`.
    pub fn new(blockchain: Blockchain, genesis_config: GenesisConfig) -> Self {
        Self {
            blockchain,
            runtimes: vec![],
            genesis_config,
        }
    }

    /// Adds a runtime with the specified identifier and returns a modified `Self` object for
    /// further chaining.
    pub fn with_runtime(mut self, runtime: impl Into<RuntimeInstance>) -> Self {
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
    pub fn build(self) -> Result<BlockchainMut, failure::Error> {
        let mut blockchain = BlockchainMut {
            dispatcher: Dispatcher::new(&self.blockchain, self.runtimes),
            inner: self.blockchain,
        };

        // If genesis block had been already created just restores dispatcher state from database
        // otherwise creates genesis block with the given specification.
        let snapshot = blockchain.snapshot();
        let has_genesis_block = !Schema::new(&snapshot).block_hashes_by_height().is_empty();

        if has_genesis_block {
            blockchain.dispatcher.restore_state(&snapshot)?;
        } else {
            blockchain.create_genesis_block(self.genesis_config)?;
        };
        Ok(blockchain)
    }
}

#[cfg(test)]
mod tests {
    use futures::sync::mpsc;

    use super::*;
    use crate::{
        blockchain::{
            config::{GenesisConfigBuilder, InstanceInitParams},
            tests::ServiceGoodImpl as SampleService,
            ConsensusConfig,
        },
        helpers::{generate_testnet_config, Height},
        runtime::rust::{RustRuntime, ServiceFactory},
    };

    #[test]
    fn finalize_without_genesis_block() {
        let config = generate_testnet_config(1, 0)[0].clone();
        let rust_runtime = RustRuntime::new(mpsc::channel(0).0);
        let genesis_config = GenesisConfigBuilder::with_consensus_config(config.consensus).build();
        let blockchain = Blockchain::build_for_tests()
            .into_mut(genesis_config)
            .with_runtime(rust_runtime)
            .build()
            .unwrap();

        let access = blockchain.snapshot();
        assert_eq!(Schema::new(access.as_ref()).height(), Height(0));
        // TODO check dispatcher schema.
    }

    // Attempts to create blockchain for particular Rust services and its instances assuming all of
    // these are builtin services.
    fn test_finalizing_services(
        services: Vec<Box<dyn ServiceFactory>>,
        instances: Vec<impl Into<InstanceInitParams>>,
    ) {
        let config = generate_testnet_config(1, 0)[0].clone();
        let rust_runtime = services
            .into_iter()
            .fold(RustRuntime::new(mpsc::channel(0).0), |runtime, factory| {
                runtime.with_factory(factory)
            });

        let genesis_config = instances
            .into_iter()
            .fold(
                GenesisConfigBuilder::with_consensus_config(config.consensus),
                |builder, instance| {
                    let instance = instance.into();
                    builder
                        .with_artifact(instance.instance_spec.artifact.clone())
                        .with_instance(instance)
                },
            )
            .build();

        Blockchain::build_for_tests()
            .into_mut(genesis_config)
            .with_runtime(rust_runtime)
            .build()
            .unwrap();
    }

    #[test]
    #[should_panic(expected = "already used")]
    fn finalize_duplicate_services() {
        let sample_service = SampleService;
        let instance = sample_service
            .artifact_id()
            .into_default_instance(0, "sample");
        test_finalizing_services(
            vec![sample_service.into()],
            vec![instance.clone(), instance],
        );
    }

    #[test]
    #[should_panic(expected = "already used")]
    fn finalize_services_with_duplicate_names() {
        let sample_service = SampleService;
        let artifact = sample_service.artifact_id();
        let instances = vec![
            artifact.clone().into_default_instance(0, "sample"),
            artifact.into_default_instance(1, "sample"),
        ];
        test_finalizing_services(vec![sample_service.into()], instances);
    }

    #[test]
    #[should_panic(expected = "already used")]
    fn finalize_services_with_duplicate_ids() {
        let sample_service = SampleService;
        let artifact = sample_service.artifact_id();
        let instances = vec![
            artifact.clone().into_default_instance(0, "sample"),
            artifact.into_default_instance(0, "other-sample"),
        ];
        test_finalizing_services(vec![sample_service.into()], instances);
    }

    #[test]
    #[should_panic(expected = "Consensus configuration must have at least one validator")]
    fn finalize_invalid_consensus_config() {
        let consensus_config = ConsensusConfig::default();
        let rust_runtime = RustRuntime::new(mpsc::channel(0).0);
        let genesis_config = GenesisConfigBuilder::with_consensus_config(consensus_config).build();
        Blockchain::build_for_tests()
            .into_mut(genesis_config)
            .with_runtime(rust_runtime)
            .build()
            .unwrap();
    }
}
