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

//! The module containing building blocks for creating blockchains powered by the Exonum framework.

pub use exonum_merkledb::Error as FatalError;

pub use crate::runtime::{
    error::{ErrorKind as ExecutionErrorKind, ExecutionStatus},
    ExecutionError,
};

pub use self::{
    block::{Block, BlockProof},
    builder::{BlockchainBuilder, InstanceCollection},
    config::{ConsensusConfig, StoredConfiguration, ValidatorKeys},
    genesis::GenesisConfig,
    schema::{IndexCoordinates, IndexOwner, Schema, TxLocation},
};

pub mod config;

use exonum_merkledb::{
    Database, Fork, IndexAccess, ObjectHash, Patch, Result as StorageResult, Snapshot,
};
use futures::{sync::mpsc, Future, Sink};

use std::{
    collections::HashMap,
    iter, panic,
    sync::{Arc, Mutex, MutexGuard},
};

use crate::{
    crypto::{Hash, PublicKey, SecretKey},
    events::InternalRequest,
    helpers::{Height, Round, ValidatorId},
    messages::{AnyTx, Connect, Message, Precommit, Verified},
    node::ApiSender,
    runtime::{dispatcher::Dispatcher, error::catch_panic},
};

mod block;
mod builder;
mod genesis;
mod schema;
#[cfg(test)]
mod tests;

/// Transaction message shortcut.
// TODO It seems that this shortcut should be removed [ECR-3222]
pub type TransactionMessage = Verified<AnyTx>;

/// Exonum blockchain instance with a certain services set and data storage.
///
/// Only nodes with an identical set of services and genesis block can be combined
/// into a single network.
#[derive(Debug, Clone)]
pub struct Blockchain {
    pub(crate) db: Arc<dyn Database>,
    // FIXME fix visibility [ECR-3222]
    #[doc(hidden)]
    pub service_keypair: (PublicKey, SecretKey),
    pub(crate) api_sender: ApiSender,
    dispatcher: Arc<Mutex<Dispatcher>>,
    internal_requests: mpsc::Sender<InternalRequest>,
}

impl Blockchain {
    /// Constructs a blockchain for the given `database` and list of `services`, also adds builtin services.
    // TODO Write proper doc string. [ECR-3275]
    pub fn new(
        database: impl Into<Arc<dyn Database>>,
        services: impl IntoIterator<Item = InstanceCollection>,
        config: GenesisConfig,
        service_keypair: (PublicKey, SecretKey),
        api_sender: ApiSender,
        internal_requests: mpsc::Sender<InternalRequest>,
    ) -> Self {
        BlockchainBuilder::new(database, config, service_keypair)
            .with_default_runtime(services)
            .finalize(api_sender, internal_requests)
            .expect("Unable to create blockchain instance")
    }

    /// Creates the blockchain instance with the specified dispatcher.
    pub(crate) fn with_dispatcher(
        db: impl Into<Arc<dyn Database>>,
        dispatcher: Dispatcher,
        service_public_key: PublicKey,
        service_secret_key: SecretKey,
        api_sender: ApiSender,
        internal_requests: mpsc::Sender<InternalRequest>,
    ) -> Self {
        Self {
            db: db.into(),
            service_keypair: (service_public_key, service_secret_key),
            api_sender,
            dispatcher: Arc::new(Mutex::new(dispatcher)),
            internal_requests,
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

    /// Returns reference to the underlying runtime dispatcher.
    pub(crate) fn dispatcher(&self) -> MutexGuard<Dispatcher> {
        self.dispatcher.lock().unwrap()
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

    /// Commits changes from the patch to the blockchain storage.
    /// See [`Fork`](../../exonum_merkledb/struct.Fork.html) for details.
    pub fn merge(&mut self, patch: Patch) -> StorageResult<()> {
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

    /// Creates and commits the genesis block with the given genesis configuration.
    fn create_genesis_block(&mut self, cfg: GenesisConfig) -> Result<(), failure::Error> {
        let config_propose = StoredConfiguration {
            previous_cfg_hash: Hash::zero(),
            actual_from: Height::zero(),
            validator_keys: cfg.validator_keys,
            consensus: cfg.consensus,
        };

        let patch = {
            let fork = self.fork();
            // Commit actual configuration
            {
                let mut schema = Schema::new(&fork);
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

    // This method is needed for EJB.
    #[doc(hidden)]
    pub fn broadcast_raw_transaction(&self, tx: AnyTx) -> Result<(), failure::Error> {
        // TODO check if service exists? [ECR-3222]

        // if !self.dispatcher.services().contains_key(&service_id) {
        //     return Err(format_err!(
        //         "Unable to broadcast transaction: no service with ID={} found",
        //         service_id
        //     ));
        // }

        self.api_sender.broadcast_transaction(Verified::from_value(
            tx,
            self.service_keypair.0,
            &self.service_keypair.1,
        ))
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
        let mut dispatcher = self.dispatcher();
        // Create fork
        let mut fork = self.fork();

        let block_hash = {
            // Get last hash.
            let last_hash = self.last_hash();
            // Save & execute transactions.
            for (index, hash) in tx_hashes.iter().enumerate() {
                Self::execute_transaction(&mut dispatcher, *hash, height, index, &mut fork)
                    // Execution could fail if the transaction
                    // cannot be deserialized or it isn't in the pool.
                    .expect("Transaction execution error.");
            }

            // Skip execution for genesis block.
            if height > Height(0) {
                dispatcher.before_commit(&mut fork);
            }

            // Get tx & state hash.
            let (tx_hash, state_hash) = {
                let schema = Schema::new(&fork);
                let state_hash = {
                    let mut sum_table = schema.state_hash_aggregator();
                    // Clear old state hash.
                    sum_table.clear();
                    // Collect all state hashes.
                    let state_hashes = dispatcher
                        .state_hash(fork.as_ref())
                        .into_iter()
                        // Add state hash of core table.
                        .chain(IndexCoordinates::locate(
                            IndexOwner::Core,
                            schema.state_hash(),
                        ));
                    // Insert state hashes into the aggregator table.
                    for (coordinate, hash) in state_hashes {
                        sum_table.put(&coordinate, hash);
                    }
                    sum_table.object_hash()
                };

                let tx_hash = schema.block_transactions(height).object_hash();

                (tx_hash, state_hash)
            };

            // Create block.
            let block = Block::new(
                proposer_id,
                height,
                tx_hashes.len() as u32,
                last_hash,
                tx_hash,
                state_hash,
            );
            trace!("execute block = {:?}", block);
            // Calculate block hash.
            let block_hash = block.object_hash();
            // Update height.
            let schema = Schema::new(&fork);
            schema.block_hashes_by_height().push(block_hash);
            // Save block.
            schema.blocks().put(&block_hash, block);

            block_hash
        };

        (block_hash, fork.into_patch())
    }

    fn execute_transaction(
        dispatcher: &mut MutexGuard<Dispatcher>,
        tx_hash: Hash,
        height: Height,
        index: usize,
        fork: &mut Fork,
    ) -> Result<(), failure::Error> {
        let transaction = {
            let new_fork = &*fork;
            let snapshot = new_fork.snapshot();
            let schema = Schema::new(snapshot);

            schema.transactions().get(&tx_hash).ok_or_else(|| {
                failure::format_err!(
                    "BUG: Cannot find transaction in database. tx: {:?}",
                    tx_hash
                )
            })?
        };

        fork.flush();

        let tx_result = catch_panic(|| dispatcher.execute(fork, tx_hash, &transaction));
        match &tx_result {
            Ok(_) => fork.flush(),
            Err(e) => {
                if e.kind == ExecutionErrorKind::Panic {
                    error!("{:?} transaction execution panicked: {:?}", transaction, e);
                } else {
                    // Unlike panic, transaction failure is a regular case. So logging the
                    // whole transaction body is an overkill: the body can be relatively big.
                    info!("{:?} transaction execution failed: {:?}", tx_hash, e);
                }
                fork.rollback();
            }
        }

        let mut schema = Schema::new(&*fork);
        schema
            .transaction_results()
            .put(&tx_hash, ExecutionStatus(tx_result));
        schema.commit_transaction(&tx_hash);
        schema.block_transactions(height).push(tx_hash);
        let location = TxLocation::new(height, index as u64);
        schema.transactions_locations().put(&tx_hash, location);
        fork.flush();
        Ok(())
    }

    /// Commits to the blockchain a new block with the indicated changes (patch),
    /// hash and Precommit messages. After that invokes `after_commit`
    /// for each service in the increasing order of their identifiers.
    pub fn commit<I>(
        &mut self,
        patch: &Patch,
        block_hash: Hash,
        precommits: I,
    ) -> Result<(), failure::Error>
    where
        I: Iterator<Item = Verified<Precommit>>,
    {
        let patch = {
            let fork = {
                let mut fork = self.db.fork();
                fork.merge(patch.clone()); // FIXME: Avoid cloning here. (ECR-1631)
                fork
            };

            {
                let mut schema = Schema::new(&fork);
                schema.precommits(&block_hash).extend(precommits);

                // Consensus messages cache is useful only during one height, so it should be
                // cleared when a new height is achieved.
                schema.consensus_messages_cache().clear();
                let txs_in_block = schema.last_block().tx_count();
                let txs_count = schema.transactions_pool_len_index().get().unwrap_or(0);
                debug_assert!(txs_count >= u64::from(txs_in_block));
                schema
                    .transactions_pool_len_index()
                    .set(txs_count - u64::from(txs_in_block));
                schema.update_transaction_count(u64::from(txs_in_block));
            }
            fork.into_patch()
        };
        self.merge(patch)?;
        // Invokes `after_commit` for each service in order of their identifiers
        let mut dispatcher = self.dispatcher();
        dispatcher.after_commit(self.snapshot(), &self.service_keypair, &self.api_sender);
        // Sends `RestartApi` request if dispatcher state was been modified.
        if dispatcher.take_modified_state() {
            self.internal_requests
                .clone()
                .send(InternalRequest::RestartApi)
                .wait()
                .map_err(|e| error!("Failed to make a request for API restart: {}", e))
                .ok();
        }
        Ok(())
    }

    // TODO move such methods into separate module. [ECR-3222]

    /// Saves the `Connect` message from a peer to the cache.
    pub(crate) fn save_peer(&mut self, pubkey: &PublicKey, peer: Verified<Connect>) {
        let fork = self.fork();
        Schema::new(&fork).peers_cache().put(pubkey, peer);
        self.merge(fork.into_patch())
            .expect("Unable to save peer to the peers cache");
    }

    /// Removes from the cache the `Connect` message from a peer.
    pub fn remove_peer_with_pubkey(&mut self, key: &PublicKey) {
        let fork = self.fork();
        Schema::new(&fork).peers_cache().remove(key);
        self.merge(fork.into_patch())
            .expect("Unable to remove peer from the peers cache");
    }

    /// Returns `Connect` messages from peers saved in the cache, if any.
    pub fn get_saved_peers(&self) -> HashMap<PublicKey, Verified<Connect>> {
        let snapshot = self.snapshot();
        Schema::new(&snapshot).peers_cache().iter().collect()
    }

    /// Saves the given raw message to the consensus messages cache.
    pub(crate) fn save_message<T: Into<Message>>(&mut self, round: Round, message: T) {
        self.save_messages(round, iter::once(message.into()));
    }

    /// Saves a collection of SignedMessage to the consensus messages cache with single access to the
    /// `Fork` instance.
    pub(crate) fn save_messages<I>(&mut self, round: Round, iter: I)
    where
        I: IntoIterator<Item = Message>,
    {
        let fork = self.fork();

        {
            let mut schema = Schema::new(&fork);
            schema.consensus_messages_cache().extend(iter);
            schema.set_consensus_round(round);
        }

        self.merge(fork.into_patch())
            .expect("Unable to save messages to the consensus cache");
    }
}
