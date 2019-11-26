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
    builder::{BlockchainBuilder, InstanceCollection, InstanceConfig},
    config::{ConsensusConfig, ValidatorKeys},
    schema::{IndexCoordinates, Schema, SchemaOrigin, TxLocation},
};

pub mod config;

use exonum_crypto::gen_keypair;
use exonum_merkledb::{
    access::RawAccess, Database, Fork, MapIndex, ObjectHash, Patch, Result as StorageResult,
    Snapshot, TemporaryDB,
};
use failure::{format_err, Error};

use std::{
    collections::{BTreeMap, HashMap},
    iter,
    sync::Arc,
};

use crate::{
    crypto::{Hash, PublicKey, SecretKey},
    helpers::{Height, Round, ValidateInput, ValidatorId},
    messages::{AnyTx, Connect, Message, Precommit, Verified},
    node::ApiSender,
    runtime::{error::catch_panic, Dispatcher},
};

mod block;
mod builder;
mod schema;
#[cfg(test)]
pub mod tests;

/// Shared Exonum blockchain instance.
///
/// This is essentially a smart pointer to shared blockchain resources (storage,
/// cryptographic keys, and a sender of transactions). It can be converted into a [`BlockchainMut`]
/// instance, which combines these resources with behavior (i.e., a set of services).
///
/// [`BlockchainMut`]: struct.BlockchainMut.html
#[derive(Debug, Clone)]
pub struct Blockchain {
    pub(crate) api_sender: ApiSender,
    db: Arc<dyn Database>,
    service_keypair: (PublicKey, SecretKey),
}

impl Blockchain {
    /// Constructs a blockchain for the given `database`.
    pub fn new(
        database: impl Into<Arc<dyn Database>>,
        service_keypair: (PublicKey, SecretKey),
        api_sender: ApiSender,
    ) -> Self {
        Self {
            db: database.into(),
            service_keypair,
            api_sender,
        }
    }

    /// Creates a non-persisting blockchain, all data in which is irrevocably lost on drop.
    ///
    /// The created blockchain cannot send transactions; an attempt to do so will result
    /// in an error.
    pub fn build_for_tests() -> Self {
        Self::new(TemporaryDB::new(), gen_keypair(), ApiSender::closed())
    }

    /// Creates a read-only snapshot of the current storage state.
    pub fn snapshot(&self) -> Box<dyn Snapshot> {
        self.db.snapshot()
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

    // TODO: remove
    // This method is needed for EJB.
    #[doc(hidden)]
    pub fn broadcast_raw_transaction(&self, tx: AnyTx) -> Result<(), Error> {
        self.api_sender.broadcast_transaction(Verified::from_value(
            tx,
            self.service_keypair.0,
            &self.service_keypair.1,
        ))
    }

    /// Returns the transactions pool size.
    pub fn pool_size(&self) -> u64 {
        Schema::new(&self.snapshot()).transactions_pool_len()
    }

    /// Returns `Connect` messages from peers saved in the cache, if any.
    pub fn get_saved_peers(&self) -> HashMap<PublicKey, Verified<Connect>> {
        let snapshot = self.snapshot();
        Schema::new(&snapshot).peers_cache().iter().collect()
    }

    /// Starts promotion into a mutable blockchain instance that can be used to process
    /// transactions and create blocks.
    #[cfg(test)]
    pub fn into_mut(self, genesis_config: ConsensusConfig) -> BlockchainBuilder {
        BlockchainBuilder::new(self, genesis_config)
    }

    /// Starts building a mutable blockchain with the genesis config, in which
    /// this node is the only validator.
    #[cfg(test)]
    pub fn into_mut_with_dummy_config(self) -> BlockchainBuilder {
        use crate::helpers::generate_testnet_config;
        use exonum_crypto::KeyPair;

        let mut config = generate_testnet_config(1, 0).pop().unwrap();
        config.keys.service = KeyPair::from(self.service_keypair.clone());
        self.into_mut(config.consensus)
    }

    /// Returns reference to the transactions sender.
    pub fn sender(&self) -> &ApiSender {
        &self.api_sender
    }

    /// Returns reference to the service key pair of the current node.
    pub fn service_keypair(&self) -> &(PublicKey, SecretKey) {
        &self.service_keypair
    }
}

/// Mutable blockchain capable of processing transactions.
///
/// `BlockchainMut` combines [`Blockchain`] resources with a service dispatcher. The resulting
/// combination cannot be cloned (unlike `Blockchain`), but can be sent across threads. It is
/// possible to extract a `Blockchain` reference from `BlockchainMut` via `AsRef` trait.
///
/// [`Blockchain`]: struct.Blockchain.html
#[derive(Debug)]
pub struct BlockchainMut {
    inner: Blockchain,
    dispatcher: Dispatcher,
}

impl AsRef<Blockchain> for BlockchainMut {
    fn as_ref(&self) -> &Blockchain {
        &self.inner
    }
}

impl BlockchainMut {
    #[cfg(test)]
    pub(crate) fn inner(&mut self) -> &mut Blockchain {
        &mut self.inner
    }

    #[cfg(test)]
    pub(crate) fn dispatcher(&mut self) -> &mut Dispatcher {
        &mut self.dispatcher
    }

    /// Returns a copy of immutable blockchain view.
    pub fn immutable_view(&self) -> Blockchain {
        self.inner.clone()
    }

    /// Creates a read-only snapshot of the current storage state.
    pub fn snapshot(&self) -> Box<dyn Snapshot> {
        self.inner.snapshot()
    }

    /// Creates a snapshot of the current storage state that can be later committed into the storage
    /// via the `merge` method.
    pub fn fork(&self) -> Fork {
        self.inner.db.fork()
    }

    /// Commits changes from the patch to the blockchain storage.
    /// See [`Fork`](../../exonum_merkledb/struct.Fork.html) for details.
    pub fn merge(&mut self, patch: Patch) -> StorageResult<()> {
        self.inner.db.merge(patch)
    }

    /// Creates and commits the genesis block with the given genesis configuration.
    // TODO: extract genesis block into separate struct [ECR-3750]
    fn create_genesis_block(
        &mut self,
        config: ConsensusConfig,
        initial_services: Vec<InstanceConfig>,
    ) -> Result<(), Error> {
        config.validate()?;
        let mut fork = self.fork();
        Schema::new(&fork).consensus_config_entry().set(config);

        // Add service instances.
        for instance_config in initial_services {
            self.dispatcher.add_builtin_service(
                &mut fork,
                instance_config.instance_spec,
                instance_config.artifact_spec.unwrap_or_default(),
                instance_config.constructor,
            )?;
        }
        // We need to activate services before calling `create_patch()`; unlike all other blocks,
        // initial services are considered immediately active in the genesis block, i.e.,
        // their state should be included into `patch` created below.
        self.dispatcher.commit_block(&mut fork);
        self.merge(fork.into_patch())?;

        let (_, patch) = self.create_patch(
            ValidatorId::zero(),
            Height::zero(),
            &[],
            &mut BTreeMap::new(),
        );
        let fork = Fork::from(patch);
        // On the other hand, we need to notify runtimes *after* the block has been created.
        // Otherwise, benign operations (e.g., calling `height()` on the core schema) will panic.
        self.dispatcher
            .notify_runtimes_about_commit(fork.snapshot_without_unflushed_changes());
        self.merge(fork.into_patch())?;

        info!(
            "GENESIS_BLOCK ====== hash={}",
            self.inner.last_hash().to_hex()
        );
        Ok(())
    }

    /// Executes the given transactions from the pool.
    /// Then collects the resulting changes from the current storage state and returns them
    /// with the hash of the resulting block.
    pub fn create_patch(
        &self,
        proposer_id: ValidatorId,
        height: Height,
        tx_hashes: &[Hash],
        tx_cache: &mut BTreeMap<Hash, Verified<AnyTx>>,
    ) -> (Hash, Patch) {
        // Create fork
        let mut fork = self.fork();
        // Get last hash.
        let last_hash = self.inner.last_hash();
        // Save & execute transactions.
        for (index, hash) in tx_hashes.iter().enumerate() {
            self.execute_transaction(*hash, height, index, &mut fork, tx_cache)
                // Execution could fail if the transaction
                // cannot be deserialized or it isn't in the pool.
                .expect("Transaction execution error");
        }

        // Skip execution for genesis block.
        if height > Height(0) {
            self.dispatcher.before_commit(&mut fork);
        }

        // Get tx & state hash.
        let schema = Schema::new(&fork);
        let state_hash = {
            let mut sum_table = schema.state_hash_aggregator();
            // Clear old state hash.
            sum_table.clear();
            // Collect all state hashes.
            let state_hashes = self
                .dispatcher
                .state_hash(fork.snapshot_without_unflushed_changes())
                .into_iter()
                // Add state hash of core table.
                .chain(IndexCoordinates::locate(
                    SchemaOrigin::Core,
                    schema.state_hash(),
                ));
            // Insert state hashes into the aggregator table.
            for (coordinate, hash) in state_hashes {
                sum_table.put(&coordinate, hash);
            }
            sum_table.object_hash()
        };
        let tx_hash = schema.block_transactions(height).object_hash();

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
        (block_hash, fork.into_patch())
    }

    fn execute_transaction(
        &self,
        tx_hash: Hash,
        height: Height,
        index: usize,
        fork: &mut Fork,
        tx_cache: &mut BTreeMap<Hash, Verified<AnyTx>>,
    ) -> Result<(), Error> {
        let schema = Schema::new(&*fork);
        let transaction = get_transaction(&tx_hash, &schema.transactions(), &tx_cache)
            .ok_or_else(|| format_err!("BUG: Cannot find transaction {:?} in database", tx_hash))?;
        fork.flush();

        let tx_result = catch_panic(|| self.dispatcher.execute(fork, tx_hash, &transaction));
        match &tx_result {
            Ok(_) => {
                fork.flush();
            }
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
        schema.commit_transaction(&tx_hash, height, transaction);
        tx_cache.remove(&tx_hash);
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
        patch: Patch,
        block_hash: Hash,
        precommits: I,
        tx_cache: &mut BTreeMap<Hash, Verified<AnyTx>>,
    ) -> Result<(), Error>
    where
        I: IntoIterator<Item = Verified<Precommit>>,
    {
        let mut fork: Fork = patch.into();
        let mut schema = Schema::new(&fork);
        schema.precommits(&block_hash).extend(precommits);
        // Consensus messages cache is useful only during one height, so it should be
        // cleared when a new height is achieved.
        schema.consensus_messages_cache().clear();
        let txs_in_block = schema.last_block().tx_count();
        schema.update_transaction_count(u64::from(txs_in_block));

        let tx_hashes = tx_cache.keys().cloned().collect::<Vec<Hash>>();
        for tx_hash in tx_hashes {
            if let Some(tx) = tx_cache.remove(&tx_hash) {
                if !schema.transactions().contains(&tx_hash) {
                    schema.add_transaction_into_pool(tx);
                }
            }
        }

        self.dispatcher.commit_block_and_notify_runtimes(&mut fork);
        self.merge(fork.into_patch())?;
        Ok(())
    }

    /// Adds a transaction into pool of uncommitted transactions.
    ///
    /// Unlike the corresponding method in the core schema, this method checks if the
    /// added transactions are already known to the node and does nothing if it is.
    /// Thus, it is safe to call this method without verifying that the transactions
    /// are not in the pool and are not committed.
    #[doc(hidden)] // used by testkit, should not be used anywhere else
    pub fn add_transactions_into_pool(
        &mut self,
        // ^-- mutable reference taken for future compatibility.
        transactions: impl IntoIterator<Item = Verified<AnyTx>>,
    ) {
        Self::add_transactions_into_db_pool(self.inner.db.as_ref(), transactions);
    }

    /// Same as `add_transactions_into_pool()`, but accepting a database handle instead
    /// of the `BlockchainMut` instance. Beware that accesses to database need to be synchronized
    /// across threads.
    #[doc(hidden)] // used by testkit, should not be used anywhere else
    pub fn add_transactions_into_db_pool<Db: Database + ?Sized>(
        db: &Db,
        transactions: impl IntoIterator<Item = Verified<AnyTx>>,
    ) {
        let fork = db.fork();
        let mut schema = Schema::new(&fork);
        for transaction in transactions {
            if !schema.transactions().contains(&transaction.object_hash()) {
                schema.add_transaction_into_pool(transaction);
            }
        }
        db.merge(fork.into_patch())
            .expect("Cannot update transaction pool");
    }

    /// Performs several shallow checks that transaction is correct.
    ///
    /// Returned `Ok(())` value doesn't necessarily mean that transaction is correct and will be
    /// executed successfully, but returned `Err(..)` value means that this transaction is
    /// **obviously** incorrect and should be declined as early as possible.
    pub(crate) fn check_tx(&self, tx: &Verified<AnyTx>) -> Result<(), ExecutionError> {
        self.dispatcher.check_tx(tx)
    }

    /// Shuts down the dispatcher. This should be the last operation performed on this instance.
    pub fn shutdown(&mut self) {
        self.dispatcher.shutdown();
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
        let mut schema = Schema::new(&fork);
        schema.consensus_messages_cache().extend(iter);
        schema.set_consensus_round(round);
        self.merge(fork.into_patch())
            .expect("Unable to save messages to the consensus cache");
    }

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
}

/// Return transaction from persistent pool. If transaction is not present in pool, try
/// to return it from transactions cache.
pub(crate) fn get_transaction<T: RawAccess>(
    hash: &Hash,
    txs: &MapIndex<T, Hash, Verified<AnyTx>>,
    tx_cache: &BTreeMap<Hash, Verified<AnyTx>>,
) -> Option<Verified<AnyTx>> {
    txs.get(&hash).or_else(|| tx_cache.get(&hash).cloned())
}

/// Check that transaction exists in the persistent pool or in the transaction cache.
pub(crate) fn contains_transaction<T: RawAccess>(
    hash: &Hash,
    txs: &MapIndex<T, Hash, Verified<AnyTx>>,
    tx_cache: &BTreeMap<Hash, Verified<AnyTx>>,
) -> bool {
    txs.contains(&hash) || tx_cache.contains_key(&hash)
}
