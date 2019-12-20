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
