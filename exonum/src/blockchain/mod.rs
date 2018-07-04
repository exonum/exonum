// Copyright 2018 The Exonum Team
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

//! The module containing building blocks for creating blockchains powered by
//! the Exonum framework.
//!
//! Services are the main extension point for the Exonum framework. To create
//! your service on top of Exonum blockchain you need to perform the following steps:
//!
//! - Define your own information schema.
//! - Create one or more transaction types using the [`transactions!`] macro and
//!   implement the [`Transaction`] trait for them.
//! - Create a data structure implementing the [`Service`] trait.
//! - Write API handlers for the service, if required.
//!
//! You may consult [the service creation tutorial][doc:create-service] for a detailed
//! instruction on how to create services.
//!
//! [`transactions!`]: ../macro.transactions.html
//! [`Transaction`]: ./trait.Transaction.html
//! [`Service`]: ./trait.Service.html
//! [doc:create-service]: https://exonum.com/doc/get-started/create-service

pub use self::block::{Block, BlockProof, SCHEMA_MAJOR_VERSION};
pub use self::config::{ConsensusConfig, StoredConfiguration, ValidatorKeys};
pub use self::genesis::GenesisConfig;
pub use self::schema::{Schema, TxLocation};
pub use self::service::{ApiContext, Service, ServiceContext, SharedNodeState};
pub use self::transaction::{
    ExecutionError, ExecutionResult, Transaction, TransactionError, TransactionErrorType,
    TransactionMessage, TransactionResult, TransactionSet,
};

pub mod config;

use byteorder::{ByteOrder, LittleEndian};
use failure;
use mount::Mount;
use vec_map::VecMap;

use std::collections::{BTreeMap, HashMap};
use std::error::Error as StdError;
use std::net::SocketAddr;
use std::ops::Deref;
use std::sync::Arc;
use std::{fmt, iter, mem, panic};

use crypto::{self, CryptoHash, Hash, PublicKey, SecretKey};
use encoding::Error as MessageError;
use helpers::{Height, Round, ValidatorId};
use messages::{
    Connect, Message, Precommit, Protocol, ProtocolMessage, RawTransaction, SignedMessage,
};
use node::ApiSender;
use storage::{Database, Error, Fork, Patch, Snapshot};

mod block;
mod genesis;
mod schema;
mod service;
#[macro_use]
mod transaction;
#[cfg(test)]
mod tests;

const CORE_SERVICE: u16 = 0;

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
    pub fn new<D: Into<Arc<Database>>>(
        storage: D,
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

    /// Returns the `VecMap` for all services. This is a map which
    /// contains service identifiers and service interfaces. The VecMap
    /// allows proceeding from the service identifier to the service itself.
    pub fn service_map(&self) -> &Arc<VecMap<Box<Service>>> {
        &self.service_map
    }

    /// Creates a read-only snapshot of the current storage state.
    pub fn snapshot(&self) -> Box<Snapshot> {
        self.db.snapshot()
    }

    /// Creates a snapshot of the current storage state that can be later committed into the storage
    /// via the `merge` method.
    pub fn fork(&self) -> Fork {
        self.db.fork()
    }

    /// Tries to create a `Transaction` object from the given raw message.
    /// A raw message can be converted into a `Transaction` object only
    /// if the following conditions are met:
    ///
    /// - Blockchain has a service with the `service_id` of the given raw message.
    /// - Service can deserialize the given raw message.
    pub fn tx_from_raw(
        &self,
        raw: &Message<RawTransaction>,
    ) -> Result<Box<Transaction>, MessageError> {
        let id = raw.service_id() as usize;
        let service = self.service_map
            .get(id)
            .ok_or_else(|| MessageError::from("Service not found."))?;
        service.tx_from_raw(raw.deref().clone())
    }

    /// Commits changes from the patch to the blockchain storage.
    /// See [`Fork`](../storage/struct.Fork.html) for details.
    pub fn merge(&mut self, patch: Patch) -> Result<(), Error> {
        self.db.merge(patch)
    }

    /// Returns the hash of the latest committed block.
    ///
    /// # Panics
    ///
    /// If the genesis block was not committed.
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
    /// If the genesis block was not committed.
    pub fn last_block(&self) -> Block {
        Schema::new(&self.snapshot()).last_block()
    }

    /// Creates and commits the genesis block with the given genesis configuration
    /// if the blockchain has not been initialized.
    pub fn initialize(&mut self, cfg: GenesisConfig) -> Result<(), Error> {
        let has_genesis_block = !Schema::new(&self.snapshot())
            .block_hashes_by_height()
            .is_empty();
        if !has_genesis_block {
            self.create_genesis_block(cfg)?;
        }
        Ok(())
    }

    /// Creates and commits the genesis block with the given genesis configuration.
    fn create_genesis_block(&mut self, cfg: GenesisConfig) -> Result<(), Error> {
        let mut config_propose = StoredConfiguration {
            previous_cfg_hash: Hash::zero(),
            actual_from: Height::zero(),
            validator_keys: cfg.validator_keys,
            consensus: cfg.consensus,
            services: BTreeMap::new(),
            majority_count: None,
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
            self.create_patch(ValidatorId::zero(), Height::zero(), &[])
                .1
        };
        self.merge(patch)?;
        Ok(())
    }

    /// Helper function to map a tuple (`u16`, `u16`) of service table coordinates
    /// to a 32-byte value to be used as the `ProofMapIndex` key (it currently
    /// supports only fixed size keys). The `hash` function is used to distribute
    /// keys uniformly (compared to padding).
    /// # Arguments
    ///
    /// * `service_id` - `service_id` as returned by instance of type of
    /// `Service` trait
    /// * `table_idx` - index of service table in `Vec`, returned by the
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

    /// Executes the given transactions from the pool.
    /// Then collects the resulting changes from the current storage state and returns them
    /// with the hash of the resulting block.
    pub fn create_patch(
        &self,
        proposer_id: ValidatorId,
        height: Height,
        tx_hashes: &[Hash],
    ) -> (Hash, Patch) {
        // Create fork
        let mut fork = self.fork();

        let block_hash = {
            // Get last hash.
            let last_hash = self.last_hash();
            // Save & execute transactions.
            for (index, hash) in tx_hashes.iter().enumerate() {
                self.execute_transaction(*hash, height, index, &mut fork)
                    // Execution could fail if the transaction
                    // cannot be deserialized or it isn't in the pool.
                    .expect("Transaction not found in the database.");
            }

            // Invoke execute method for all services.
            for service in self.service_map.values() {
                // Skip execution for genesis block.
                if height > Height(0) {
                    service_execute(service.as_ref(), &mut fork);
                }
            }

            // Get tx & state hash.
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
                    sum_table.merkle_root()
                };

                let tx_hash = schema.block_transactions(height).merkle_root();

                (tx_hash, state_hash)
            };

            // Create block.
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
            // Calculate block hash.
            let block_hash = block.hash();
            // Update height.
            let mut schema = Schema::new(&mut fork);
            schema.block_hashes_by_height_mut().push(block_hash);
            // Save block.
            schema.blocks_mut().put(&block_hash, block);

            block_hash
        };

        (block_hash, fork.into_patch())
    }

    fn execute_transaction(
        &self,
        tx_hash: Hash,
        height: Height,
        index: usize,
        fork: &mut Fork,
    ) -> Result<(), failure::Error> {
        let tx = {
            let schema = Schema::new(&fork);

            let tx = schema
                .transactions()
                .get(&tx_hash)
                .ok_or_else(|| failure::err_msg("BUG: Cannot find transaction in database."))?;

            self.tx_from_raw(&tx).or_else(|error| {
                Err(failure::err_msg(format!(
                    "{}, tx: {:?}",
                    error.description(),
                    tx_hash
                )))
            })?
        };

        fork.checkpoint();

        let catch_result = panic::catch_unwind(panic::AssertUnwindSafe(|| tx.execute(fork)));

        let tx_result = match catch_result {
            Ok(execution_result) => {
                match execution_result {
                    Ok(()) => {
                        fork.commit();
                    }
                    Err(ref e) => {
                        // Unlike panic, transaction failure isn't that rare, so logging the
                        // whole transaction body is an overkill: it can be relatively big.
                        info!("{:?} transaction execution failed: {:?}", tx_hash, e);
                        fork.rollback();
                    }
                }
                execution_result.map_err(TransactionError::from)
            }
            Err(err) => {
                if err.is::<Error>() {
                    // Continue panic unwind if the reason is StorageError.
                    panic::resume_unwind(err);
                }
                fork.rollback();
                error!("{:?} transaction execution panicked: {:?}", tx, err);
                Err(TransactionError::from_panic(&err))
            }
        };

        let mut schema = Schema::new(fork);
        schema.transaction_results_mut().put(&tx_hash, tx_result);
        schema.commit_transaction(&tx_hash);
        schema.block_transactions_mut(height).push(tx_hash);
        let location = TxLocation::new(height, index as u64);
        schema.transactions_locations_mut().put(&tx_hash, location);
        Ok(())
    }

    /// Commits to the blockchain a new block with the indicated changes (patch),
    /// hash and Precommit messages. After that invokes `handle_commit`
    /// for each service in the increasing order of their identifiers.
    #[cfg_attr(feature = "flame_profile", flame)]
    pub fn commit<I>(&mut self, patch: &Patch, block_hash: Hash, precommits: I) -> Result<(), Error>
    where
        I: Iterator<Item = Message<Precommit>>,
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

                // Consensus messages cache is useful only during one height, so it should be
                // cleared when a new height is achieved.
                schema.consensus_messages_cache_mut().clear();
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

    /// Returns the `Mount` object that aggregates public API handlers.
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

    /// Returns the `Mount` object that aggregates private API handlers.
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

    /// Saves the `Connect` message from a peer to the cache.
    pub fn save_peer(&mut self, pubkey: &PublicKey, peer: Message<Connect>) {
        let mut fork = self.fork();

        {
            let mut schema = Schema::new(&mut fork);
            schema.peers_cache_mut().put(pubkey, peer);
        }

        self.merge(fork.into_patch())
            .expect("Unable to save peer to the peers cache");
    }

    /// Removes from the cache the `Connect` message from a peer.
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

        self.merge(fork.into_patch())
            .expect("Unable to remove peer from the peers cache");
    }

    /// Returns `Connect` messages from peers saved in the cache, if any.
    pub fn get_saved_peers(&self) -> HashMap<PublicKey, Message<Connect>> {
        let schema = Schema::new(self.snapshot());
        let peers_cache = schema.peers_cache();
        let it = peers_cache.iter().map(|(k, v)| (k, v.clone()));
        it.collect()
    }

    /// Saves the given raw message to the consensus messages cache.
    pub fn save_message<T: ProtocolMessage>(&mut self, round: Round, raw: Message<T>) {
        self.save_messages(round, iter::once(raw.downgrade()));
    }

    /// Saves a collection of SignedMessage to the consensus messages cache with single access to the
    /// `Fork` instance.
    pub fn save_messages<I>(&mut self, round: Round, iter: I)
    where
        I: IntoIterator<Item = Message<Protocol>>,
    {
        let mut fork = self.fork();

        {
            let mut schema = Schema::new(&mut fork);
            schema.consensus_messages_cache_mut().extend(iter);
            schema.set_consensus_round(round);
        }

        self.merge(fork.into_patch())
            .expect("Unable to save messages to the consensus cache");
    }
}

fn service_execute(service: &Service, fork: &mut Fork) {
    fork.checkpoint();
    match panic::catch_unwind(panic::AssertUnwindSafe(|| service.execute(fork))) {
        Ok(..) => fork.commit(),
        Err(err) => {
            if err.is::<Error>() {
                // Continue panic unwind if the reason is StorageError.
                panic::resume_unwind(err);
            }
            fork.rollback();
            error!(
                "{} service execute failed with error: {:?}",
                service.service_name(),
                err
            );
        }
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
