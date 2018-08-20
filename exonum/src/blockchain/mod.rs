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

pub use self::{
    block::{Block, BlockProof}, config::{ConsensusConfig, StoredConfiguration, ValidatorKeys},
    genesis::GenesisConfig, schema::{Schema, TxLocation},
    service::{Service, ServiceContext, SharedNodeState},
    transaction::{
        ExecutionError, ExecutionResult, Transaction, TransactionError, TransactionErrorType,
        TransactionResult, TransactionSet,
    },
};

pub mod config;

use byteorder::{ByteOrder, LittleEndian};
use failure;
use vec_map::VecMap;

use std::{
    collections::{BTreeMap, HashMap}, error::Error as StdError, fmt, iter, mem, net::SocketAddr,
    panic, sync::Arc,
};

use crypto::{self, CryptoHash, Hash, PublicKey, SecretKey};
use encoding::Error as MessageError;
use helpers::{Height, Round, ValidatorId};
use messages::{Connect, Precommit, RawMessage, CONSENSUS as CORE_SERVICE};
use node::ApiSender;
use storage::{self, Database, Error, Fork, Patch, Snapshot};

mod block;
mod genesis;
mod schema;
mod service;
#[macro_use]
mod transaction;
#[cfg(test)]
mod tests;

/// Exonum blockchain instance with a certain services set and data storage.
///
/// Only nodes with an identical set of services and genesis block can be combined
/// into a single network.
pub struct Blockchain {
    db: Arc<dyn Database>,
    service_map: Arc<VecMap<Box<dyn Service>>>,
    pub(crate) service_keypair: (PublicKey, SecretKey),
    pub(crate) api_sender: ApiSender,
}

impl Blockchain {
    /// Constructs a blockchain for the given `storage` and list of `services`.
    pub fn new<D: Into<Arc<dyn Database>>>(
        storage: D,
        services: Vec<Box<dyn Service>>,
        service_public_key: PublicKey,
        service_secret_key: SecretKey,
        api_sender: ApiSender,
    ) -> Self {
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

        Self {
            db: storage.into(),
            service_map: Arc::new(service_map),
            service_keypair: (service_public_key, service_secret_key),
            api_sender,
        }
    }

    /// Recreates the blockchain to reuse with a sandbox.
    #[doc(hidden)]
    pub fn clone_with_api_sender(&self, api_sender: ApiSender) -> Self {
        Self {
            api_sender,
            ..self.clone()
        }
    }

    /// Returns the `VecMap` for all services. This is a map which
    /// contains service identifiers and service interfaces. The VecMap
    /// allows proceeding from the service identifier to the service itself.
    pub fn service_map(&self) -> &Arc<VecMap<Box<dyn Service>>> {
        &self.service_map
    }

    /// Creates a read-only snapshot of the current storage state.
    pub fn snapshot(&self) -> Box<dyn Snapshot> {
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
    pub fn tx_from_raw(&self, raw: RawMessage) -> Result<Box<dyn Transaction>, MessageError> {
        let id = raw.service_id() as usize;
        let service = self.service_map
            .get(id)
            .ok_or_else(|| MessageError::from("Service not found."))?;
        service.tx_from_raw(raw)
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
    pub fn last_block(&self) -> Block {
        Schema::new(&self.snapshot()).last_block()
    }

    /// Creates and commits the genesis block with the given genesis configuration
    /// if the blockchain has not been initialized.
    ///
    /// # Panics
    ///
    /// * If the genesis block was not committed.
    /// * If storage version is not specified or not supported.
    pub fn initialize(&mut self, cfg: GenesisConfig) -> Result<(), Error> {
        let has_genesis_block = !Schema::new(&self.snapshot())
            .block_hashes_by_height()
            .is_empty();
        if has_genesis_block {
            self.assert_storage_version();
        } else {
            self.initialize_metadata();
            self.create_genesis_block(cfg)?;
        }
        Ok(())
    }

    /// Initialized node-local metadata.
    fn initialize_metadata(&mut self) {
        let mut fork = self.db.fork();
        storage::StorageMetadata::write_current(&mut fork);
        if self.merge(fork.into_patch()).is_ok() {
            info!(
                "Storage version successfully initialized with value [{}].",
                storage::StorageMetadata::read(&self.db.snapshot()).unwrap(),
            )
        } else {
            panic!("Could not set database version.")
        }
    }

    /// Checks if storage version is supported.
    ///
    /// # Panics
    ///
    /// Panics if version is not supported or is not specified.
    fn assert_storage_version(&self) {
        match storage::StorageMetadata::read(self.db.snapshot()) {
            Ok(ver) => info!("Storage version is supported with value [{}].", ver),
            Err(e) => panic!("{}", e),
        }
    }

    /// Creates and commits the genesis block with the given genesis configuration.
    fn create_genesis_block(&mut self, cfg: GenesisConfig) -> Result<(), Error> {
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
                    // TODO create genesis block for MemoryDB and compare it hash with zero block. (ECR-1630)
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
                    before_commit(service.as_ref(), &mut fork);
                }
            }

            // Get tx & state hash.
            let (tx_hash, state_hash) = {
                let state_hashes = {
                    let schema = Schema::new(&fork);

                    let vec_core_state = schema.core_state_hash();
                    let mut state_hashes = Vec::new();

                    for (idx, core_table_hash) in vec_core_state.into_iter().enumerate() {
                        let key = Self::service_table_unique_key(CORE_SERVICE, idx);
                        state_hashes.push((key, core_table_hash));
                    }

                    for service in self.service_map.values() {
                        let service_id = service.service_id();
                        let vec_service_state = service.state_hash(&fork);
                        for (idx, service_table_hash) in vec_service_state.into_iter().enumerate() {
                            let key = Self::service_table_unique_key(service_id, idx);
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
        let (tx, service_name) = {
            let schema = Schema::new(&fork);

            let tx = schema
                .transactions()
                .get(&tx_hash)
                .ok_or_else(|| failure::err_msg("BUG: Cannot find transaction in database."))?;

            let service_name = self.service_map
                .get(tx.service_id() as usize)
                .ok_or_else(|| failure::err_msg("Service not found."))?
                .service_name();

            let tx = self.tx_from_raw(tx).or_else(|error| {
                Err(failure::err_msg(format!(
                    "Service <{}>: {}, tx: {:?}",
                    service_name,
                    error.description(),
                    tx_hash
                )))
            })?;

            (tx, service_name)
        };

        fork.checkpoint();

        let catch_result = panic::catch_unwind(panic::AssertUnwindSafe(|| tx.execute(fork)));

        let tx_result = TransactionResult(match catch_result {
            Ok(execution_result) => {
                match execution_result {
                    Ok(()) => {
                        fork.commit();
                    }
                    Err(ref e) => {
                        // Unlike panic, transaction failure isn't that rare, so logging the
                        // whole transaction body is an overkill: it can be relatively big.
                        info!(
                            "Service <{}>: {:?} transaction execution failed: {:?}",
                            service_name, tx_hash, e
                        );
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
                error!(
                    "Service <{}>: {:?} transaction execution panicked: {:?}",
                    service_name, tx, err
                );
                Err(TransactionError::from_panic(&err))
            }
        });

        let mut schema = Schema::new(fork);
        schema.transaction_results_mut().put(&tx_hash, tx_result);
        schema.commit_transaction(&tx_hash);
        schema.block_transactions_mut(height).push(tx_hash);
        let location = TxLocation::new(height, index as u64);
        schema.transactions_locations_mut().put(&tx_hash, location);
        Ok(())
    }

    /// Commits to the blockchain a new block with the indicated changes (patch),
    /// hash and Precommit messages. After that invokes `after_commit`
    /// for each service in the increasing order of their identifiers.
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
                fork.merge(patch.clone()); // FIXME: Avoid cloning here. (ECR-1631)
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
                let txs_in_block = schema.last_block().tx_count();
                let txs_count = schema.transactions_pool_len_index().get().unwrap_or(0);
                debug_assert!(txs_count >= u64::from(txs_in_block));
                schema
                    .transactions_pool_len_index_mut()
                    .set(txs_count - u64::from(txs_in_block));
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

        // Invokes `after_commit` for each service in order of their identifiers
        for service in self.service_map.values() {
            service.after_commit(&context);
        }
        Ok(())
    }

    /// Saves the `Connect` message from a peer to the cache.
    pub(crate) fn save_peer(&mut self, pubkey: &PublicKey, peer: Connect) {
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
    pub fn get_saved_peers(&self) -> HashMap<PublicKey, Connect> {
        let schema = Schema::new(self.snapshot());
        let peers_cache = schema.peers_cache();
        let it = peers_cache.iter().map(|(k, v)| (k, v.clone()));
        it.collect()
    }

    /// Saves the given raw message to the consensus messages cache.
    pub(crate) fn save_message(&mut self, round: Round, raw: &RawMessage) {
        self.save_messages(round, iter::once(raw.clone()));
    }

    /// Saves a collection of RawMessage to the consensus messages cache with single access to the
    /// `Fork` instance.
    pub(crate) fn save_messages<I>(&mut self, round: Round, iter: I)
    where
        I: IntoIterator<Item = RawMessage>,
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

fn before_commit(service: &dyn Service, fork: &mut Fork) {
    fork.checkpoint();
    match panic::catch_unwind(panic::AssertUnwindSafe(|| service.before_commit(fork))) {
        Ok(..) => fork.commit(),
        Err(err) => {
            if err.is::<Error>() {
                // Continue panic unwind if the reason is StorageError.
                panic::resume_unwind(err);
            }
            fork.rollback();
            error!(
                "{} service before_commit failed with error: {:?}",
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
    fn clone(&self) -> Self {
        Self {
            db: Arc::clone(&self.db),
            service_map: Arc::clone(&self.service_map),
            api_sender: self.api_sender.clone(),
            service_keypair: self.service_keypair.clone(),
        }
    }
}
