// Copyright 2017 The Exonum Team
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

//! The module containing building blocks for creating blockchains powered by the
//! Exonum framework.
//!
//! Services are the main extension point for the Exonum framework. To create your service on
//! top of Exonum blockchain you need to do the following:
//!
//! - Define your own information schema.
//! - Create one or more transaction types using the [`message!`] macro and implement
//!   the [`Transaction`] trait for them.
//! - Create a data structure implementing the [`Service`] trait.
//! - Optionally you can write API handlers for the service.
//!
//! You may consult [the service creation tutorial][doc:create-service] for a more detailed
//! manual how to create services.
//!
//! [`message!`]: ../macro.message.html
//! [`Transaction`]: ./trait.Transaction.html
//! [`Service`]: ./trait.Service.html
//! [doc:create-service]: https://exonum.com/doc/get-started/create-service

use std::sync::Arc;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::mem;
use std::fmt;
use std::panic;
use std::net::SocketAddr;

use vec_map::VecMap;
use byteorder::{ByteOrder, LittleEndian};
use mount::Mount;

use crypto::{self, Hash, PublicKey, SecretKey};
use messages::{CONSENSUS as CORE_SERVICE, Precommit, RawMessage, Connect};
use storage::{Database, Error, Fork, Patch, Snapshot};
use helpers::{Height, ValidatorId};
use node::ApiSender;

pub use self::block::{Block, BlockProof, SCHEMA_MAJOR_VERSION};
pub use self::schema::{gen_prefix, Schema, TxLocation};
pub use self::genesis::GenesisConfig;
pub use self::config::{ConsensusConfig, StoredConfiguration, TimeoutAdjusterConfig, ValidatorKeys};
pub use self::service::{ApiContext, Service, ServiceContext, SharedNodeState, Transaction};

mod block;
mod schema;
mod genesis;
mod service;
#[cfg(test)]
mod tests;

pub mod config;

/// Exonum blockchain instance with the concrete services set and data storage.
/// Only blockchains with the identical set of services and genesis block can be combined
/// into the single network.
pub struct Blockchain {
    db: Arc<Database>,
    service_map: Arc<VecMap<Box<Service>>>,
    service_keypair: (PublicKey, SecretKey),
    api_sender: ApiSender,
}

impl Blockchain {
    /// Constructs a blockchain for the given `storage` and list of `services`.
    pub fn new(
        storage: Box<Database>,
        services: Vec<Box<Service>>,
        service_public_key: PublicKey,
        service_secret_key: SecretKey,
        api_sender: ApiSender,
    ) -> Blockchain {
        let mut service_map = VecMap::new();
        for service in services {
            let id = service.service_id() as usize;
            if service_map.contains_key(id) {
                panic!(
                    "Services have already contain service with id={}, please change it.",
                    id
                );
            }
            service_map.insert(id, service);
        }

        Blockchain {
            db: storage.into(),
            service_map: Arc::new(service_map),
            service_keypair: (service_public_key, service_secret_key),
            api_sender,
        }
    }

    /// Recreates the blockchain to reuse with a sandbox.
    #[doc(hidden)]
    pub fn clone_with_api_sender(&self, api_sender: ApiSender) -> Blockchain {
        Blockchain {
            api_sender,
            ..self.clone()
        }
    }

    /// Returns service `VecMap` for all our services.
    pub fn service_map(&self) -> &Arc<VecMap<Box<Service>>> {
        &self.service_map
    }

    /// Creates a readonly snapshot of the current storage state.
    pub fn snapshot(&self) -> Box<Snapshot> {
        self.db.snapshot()
    }

    /// Creates snapshot of the current storage state that can be later committed into storage
    /// via `merge` method.
    pub fn fork(&self) -> Fork {
        self.db.fork()
    }

    /// Tries to create a `Transaction` object from the given raw message.
    /// Raw message can be converted into `Transaction` object only
    /// if following conditions are met.
    ///
    /// - Blockchain has service with the `service_id` of given raw message.
    /// - Service can deserialize given raw message.
    pub fn tx_from_raw(&self, raw: RawMessage) -> Option<Box<Transaction>> {
        let id = raw.service_id() as usize;
        self.service_map.get(id).and_then(|service| {
            service.tx_from_raw(raw).ok()
        })
    }

    /// Commits changes from the patch to the blockchain storage.
    /// See [`Fork`](../storage/struct.Fork.html) for details.
    pub fn merge(&mut self, patch: Patch) -> Result<(), Error> {
        self.db.merge(patch)
    }

    /// Returns the hash of latest committed block.
    ///
    /// # Panics
    ///
    /// - If the genesis block was not committed.
    pub fn last_hash(&self) -> Hash {
        Schema::new(&self.snapshot())
            .block_hashes_by_height()
            .last()
            .unwrap_or_else(Hash::default)
    }

    /// Returns the latest committed block.
    ///
    /// # Panics
    ///
    /// - If the genesis block was not committed.
    pub fn last_block(&self) -> Block {
        Schema::new(&self.snapshot()).last_block()
    }

    /// Creates and commits the genesis block for the given genesis configuration.
    pub fn create_genesis_block(&mut self, cfg: GenesisConfig) -> Result<(), Error> {
        let mut config_propose = StoredConfiguration {
            previous_cfg_hash: Hash::zero(),
            actual_from: Height::zero(),
            validator_keys: cfg.validator_keys,
            consensus: cfg.consensus,
            services: BTreeMap::new(),
        };

        let patch = {
            let mut fork = self.fork();
            // Update service tables
            for (_, service) in self.service_map.iter() {
                let cfg = service.initialize(&mut fork);
                let name = service.service_name();
                if config_propose.services.contains_key(name) {
                    panic!(
                        "Services already contain service with '{}' name, please change it",
                        name
                    );
                }
                config_propose.services.insert(name.into(), cfg);
            }
            // Commit actual configuration
            {
                let mut schema = Schema::new(&mut fork);
                if schema.block_hash_by_height(Height::zero()).is_some() {
                    // TODO create genesis block for MemoryDB and compare in hash with zero block
                    return Ok(());
                }
                schema.commit_configuration(config_propose);
            };
            self.merge(fork.into_patch())?;
            self.create_patch(ValidatorId::zero(), Height::zero(), &[], &BTreeMap::new())
                .1
        };
        self.merge(patch)?;
        Ok(())
    }

    /// Helper function to map tuple (`u16`, `u16`) of service table coordinates
    /// to 32 byte value for use as `MerklePatriciaTable` key (it currently
    /// supports only fixed size keys). `hash` function is used to distribute
    /// keys uniformly (compared to padding).
    /// # Arguments
    ///
    /// * `service_id` - `service_id` as returned by instance of type of
    /// `Service` trait
    /// * `table_idx` - index of service table in `Vec`, returned by
    /// `state_hash` method of instance of type of `Service` trait
    // also, it was the first idea around, to use `hash`
    pub fn service_table_unique_key(service_id: u16, table_idx: usize) -> Hash {
        debug_assert!(table_idx <= u16::max_value() as usize);
        let size = mem::size_of::<u16>();
        let mut vec = vec![0; 2 * size];
        LittleEndian::write_u16(&mut vec[0..size], service_id);
        LittleEndian::write_u16(&mut vec[size..2 * size], table_idx as u16);
        crypto::hash(&vec)
    }

    /// Executes the given transactions from pool.
    /// Then it collects the resulting changes from the current storage state and returns them
    /// with the hash of resulting block.
    pub fn create_patch(
        &self,
        proposer_id: ValidatorId,
        height: Height,
        tx_hashes: &[Hash],
        pool: &BTreeMap<Hash, Box<Transaction>>,
    ) -> (Hash, Patch) {
        // Create fork
        let mut fork = self.fork();

        let block_hash = {
            // Get last hash
            let last_hash = self.last_hash();
            // Save & execute transactions
            for (index, hash) in tx_hashes.iter().enumerate() {
                let tx = pool.get(hash).expect(
                    "BUG: Cannot find transaction in pool.",
                );

                fork.checkpoint();

                let r = panic::catch_unwind(panic::AssertUnwindSafe(|| { tx.execute(&mut fork); }));

                match r {
                    Ok(..) => fork.commit(),
                    Err(err) => {
                        if err.is::<Error>() {
                            // Continue panic unwind if the reason is StorageError
                            panic::resume_unwind(err);
                        }
                        fork.rollback();
                        error!("{:?} transaction execution failed: {:?}", tx, err);
                    }
                }

                let mut schema = Schema::new(&mut fork);
                schema.transactions_mut().put(hash, tx.raw().clone());
                schema.block_txs_mut(height).push(*hash);
                let location = TxLocation::new(height, index as u64);
                schema.tx_location_by_tx_hash_mut().put(hash, location);
            }

            // Get tx & state hash
            let (tx_hash, state_hash) = {
                let state_hashes = {
                    let schema = Schema::new(&fork);

                    let vec_core_state = schema.core_state_hash();
                    let mut state_hashes = Vec::new();

                    for (idx, core_table_hash) in vec_core_state.into_iter().enumerate() {
                        let key = Blockchain::service_table_unique_key(CORE_SERVICE, idx);
                        state_hashes.push((key, core_table_hash));
                    }

                    for service in self.service_map.values() {
                        let service_id = service.service_id();
                        let vec_service_state = service.state_hash(&fork);
                        for (idx, service_table_hash) in vec_service_state.into_iter().enumerate() {
                            let key = Blockchain::service_table_unique_key(service_id, idx);
                            state_hashes.push((key, service_table_hash));
                        }
                    }

                    state_hashes
                };

                let mut schema = Schema::new(&mut fork);

                let state_hash = {
                    let mut sum_table = schema.state_hash_aggregator_mut();
                    for (key, hash) in state_hashes {
                        sum_table.put(&key, hash)
                    }
                    sum_table.root_hash()
                };

                let tx_hash = schema.block_txs(height).root_hash();

                (tx_hash, state_hash)
            };

            // Create block
            let block = Block::new(
                SCHEMA_MAJOR_VERSION,
                proposer_id,
                height,
                tx_hashes.len() as u32,
                &last_hash,
                &tx_hash,
                &state_hash,
            );
            trace!("execute block = {:?}", block);
            // Eval block hash
            let block_hash = block.hash();
            // Update height
            let mut schema = Schema::new(&mut fork);
            schema.block_hashes_by_height_mut().push(block_hash);
            // Save block
            schema.blocks_mut().put(&block_hash, block);

            block_hash
        };

        (block_hash, fork.into_patch())
    }

    /// Commits to the storage block that proposes by node `State`.
    /// After that invokes `handle_commit` for each service in order of their identifiers
    /// and returns the list of transactions which which were created by the `handle_commit` event.
    #[cfg_attr(feature = "flame_profile", flame)]
    pub fn commit<'a, I>(
        &mut self,
        patch: &Patch,
        block_hash: Hash,
        precommits: I,
    ) -> Result<(), Error>
    where
        I: Iterator<Item = &'a Precommit>,
    {
        let patch = {
            let mut fork = {
                let mut fork = self.db.fork();
                fork.merge(patch.clone()); // FIXME: avoid cloning here
                fork
            };

            {
                let mut schema = Schema::new(&mut fork);
                for precommit in precommits {
                    schema.precommits_mut(&block_hash).push(precommit.clone());
                }
            }
            fork.into_patch()
        };
        self.merge(patch)?;
        // Initializes the context after merge.
        let context = ServiceContext::new(
            self.service_keypair.0,
            self.service_keypair.1.clone(),
            self.api_sender.clone(),
            self.fork(),
        );
        // Invokes `handle_commit` for each service in order of their identifiers
        for service in self.service_map.values() {
            service.handle_commit(&context);
        }
        Ok(())
    }

    /// Returns `Mount` object that aggregates public api handlers.
    pub fn mount_public_api(&self) -> Mount {
        let context = self.api_context();
        let mut mount = Mount::new();
        for service in self.service_map.values() {
            if let Some(handler) = service.public_api_handler(&context) {
                mount.mount(service.service_name(), handler);
            }
        }
        mount
    }

    /// Returns `Mount` object that aggregates private api handlers.
    pub fn mount_private_api(&self) -> Mount {
        let context = self.api_context();
        let mut mount = Mount::new();
        for service in self.service_map.values() {
            if let Some(handler) = service.private_api_handler(&context) {
                mount.mount(service.service_name(), handler);
            }
        }
        mount
    }

    fn api_context(&self) -> ApiContext {
        ApiContext::from_parts(
            self,
            self.api_sender.clone(),
            &self.service_keypair.0,
            &self.service_keypair.1,
        )
    }

    /// Saves peer to the peers cache
    pub fn save_peer(&mut self, pubkey: &PublicKey, peer: Connect) {
        let mut fork = self.fork();

        {
            let mut schema = Schema::new(&mut fork);
            schema.peers_cache_mut().put(pubkey, peer);
        }

        self.merge(fork.into_patch()).expect(
            "Unable to save peer to the peers cache",
        );
    }

    /// Removes peer from the peers cache
    pub fn remove_peer_with_addr(&mut self, addr: &SocketAddr) {
        let mut fork = self.fork();

        {
            let mut schema = Schema::new(&mut fork);
            let mut peers = schema.peers_cache_mut();
            let peer = peers.iter().find(|&(_, ref v)| v.addr() == *addr);
            if let Some(pubkey) = peer.map(|(k, _)| k) {
                peers.remove(&pubkey);
            }
        }

        self.merge(fork.into_patch()).expect(
            "Unable to remove peer from the peers cache",
        );
    }

    /// Recover cached peers if any.
    pub fn get_saved_peers(&self) -> HashMap<PublicKey, Connect> {
        let schema = Schema::new(self.snapshot());
        let peers_cache = schema.peers_cache();
        let it = peers_cache.iter().map(|(k, v)| (k, v.clone()));
        it.collect()
    }
}

impl fmt::Debug for Blockchain {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Blockchain(..)")
    }
}

impl Clone for Blockchain {
    fn clone(&self) -> Blockchain {
        Blockchain {
            db: Arc::clone(&self.db),
            service_map: Arc::clone(&self.service_map),
            api_sender: self.api_sender.clone(),
            service_keypair: self.service_keypair.clone(),
        }
    }
}
