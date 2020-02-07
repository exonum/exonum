// Copyright 2020 The Exonum Team
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

//! Building blocks for creating blockchains powered by the Exonum framework.

pub use self::{
    api_sender::{ApiSender, SendError},
    block::{AdditionalHeaders, Block, BlockHeaderKey, BlockProof, IndexProof, ProposerId},
    builder::BlockchainBuilder,
    config::{ConsensusConfig, ConsensusConfigBuilder, ValidatorKeys},
    schema::{CallInBlock, Schema, TxLocation},
};

pub mod config;

pub(crate) use crate::runtime::ExecutionError;

use exonum_crypto::{Hash, KeyPair};
use exonum_merkledb::{
    access::RawAccess, Database, Fork, MapIndex, ObjectHash, Patch, Result as StorageResult,
    Snapshot, SystemSchema, TemporaryDB,
};
use failure::Error;
use futures::Future;

use std::{collections::BTreeMap, sync::Arc};

use crate::{
    blockchain::config::GenesisConfig,
    helpers::{Height, ValidateInput, ValidatorId},
    messages::{AnyTx, Precommit, Verified},
    runtime::Dispatcher,
};

mod api_sender;
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
    service_keypair: KeyPair,
}

impl Blockchain {
    /// Constructs a blockchain for the given `database`.
    pub fn new(
        database: impl Into<Arc<dyn Database>>,
        service_keypair: impl Into<KeyPair>,
        api_sender: ApiSender,
    ) -> Self {
        Self {
            db: database.into(),
            service_keypair: service_keypair.into(),
            api_sender,
        }
    }

    /// Creates a non-persisting blockchain, all data in which is irrevocably lost on drop.
    ///
    /// The created blockchain cannot send transactions; an attempt to do so will result
    /// in an error.
    pub fn build_for_tests() -> Self {
        Self::new(TemporaryDB::new(), KeyPair::random(), ApiSender::closed())
    }

    /// Returns a reference to the database enclosed by this `Blockchain`.
    pub(crate) fn database(&self) -> &Arc<dyn Database> {
        &self.db
    }

    /// Creates a read-only snapshot of the current storage state.
    pub fn snapshot(&self) -> Box<dyn Snapshot> {
        self.db.snapshot()
    }

    /// Returns the hash of the latest committed block.
    /// If genesis block was not committed returns `Hash::zero()`.
    pub fn last_hash(&self) -> Hash {
        Schema::new(&self.snapshot())
            .block_hashes_by_height()
            .last()
            .unwrap_or_else(Hash::zero)
    }

    /// Returns the latest committed block.
    pub fn last_block(&self) -> Block {
        Schema::new(&self.snapshot()).last_block()
    }

    /// Returns the transactions pool size.
    #[doc(hidden)]
    pub fn pool_size(&self) -> u64 {
        Schema::new(&self.snapshot()).transactions_pool_len()
    }

    /// Starts promotion into a mutable blockchain instance that can be used to process
    /// transactions and create blocks.
    #[cfg(test)]
    pub fn into_mut(self, genesis_config: GenesisConfig) -> BlockchainBuilder {
        BlockchainBuilder::new(self).with_genesis_config(genesis_config)
    }

    /// Starts building a mutable blockchain with the genesis config, in which
    /// this node is the only validator.
    #[cfg(test)]
    pub fn into_mut_with_dummy_config(self) -> BlockchainBuilder {
        use self::config::GenesisConfigBuilder;

        let (mut config, _) = ConsensusConfig::for_tests(1);
        config.validator_keys[0].service_key = self.service_keypair.public_key();
        let genesis_config = GenesisConfigBuilder::with_consensus_config(config).build();
        self.into_mut(genesis_config)
    }

    /// Returns reference to the transactions sender.
    pub fn sender(&self) -> &ApiSender {
        &self.api_sender
    }

    /// Returns reference to the service key pair of the current node.
    pub fn service_keypair(&self) -> &KeyPair {
        &self.service_keypair
    }

    /// Performs several shallow checks that transaction is correct.
    ///
    /// Returned `Ok(())` value doesn't necessarily mean that transaction is correct and will be
    /// executed successfully, but returned `Err(..)` value means that this transaction is
    /// **obviously** incorrect and should be declined as early as possible.
    pub fn check_tx(snapshot: &dyn Snapshot, tx: &Verified<AnyTx>) -> Result<(), ExecutionError> {
        Dispatcher::check_tx(snapshot, tx)
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
    /// Returns a copy of immutable blockchain view.
    pub fn immutable_view(&self) -> Blockchain {
        self.inner.clone()
    }

    /// Returns a mutable reference to dispatcher.
    #[cfg(test)]
    pub(crate) fn dispatcher(&mut self) -> &mut Dispatcher {
        &mut self.dispatcher
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

    /// Commits changes from the `patch` to the blockchain storage.
    pub fn merge(&mut self, patch: Patch) -> StorageResult<()> {
        self.inner.db.merge(patch)
    }

    /// Creates and commits the genesis block with the given genesis configuration.
    ///
    /// # Panics
    ///
    /// Panics if the genesis block cannot be created.
    fn create_genesis_block(&mut self, genesis_config: GenesisConfig) {
        genesis_config
            .consensus_config
            .validate()
            .expect("Invalid consensus config");
        let mut fork = self.fork();
        // Write genesis configuration to the blockchain.
        Schema::new(&fork)
            .consensus_config_entry()
            .set(genesis_config.consensus_config);

        for spec in genesis_config.artifacts {
            Dispatcher::commit_artifact(&fork, &spec.artifact, spec.payload.clone());
            self.dispatcher
                .deploy_artifact(spec.artifact, spec.payload)
                .wait()
                .expect("Cannot deploy an artifact");
        }
        // Add service instances.
        // Note that `before_transactions` will not be invoked for services, since
        // they are added within block (and don't appear from nowhere).
        for inst in genesis_config.builtin_instances {
            self.dispatcher
                .add_builtin_service(&mut fork, inst.instance_spec, inst.constructor)
                .expect("Unable to add a builtin service");
        }
        // Activate services and persist changes.
        let patch = self.dispatcher.start_builtin_instances(fork);
        self.merge(patch).unwrap();

        // Create a new fork to collect the changes from `after_transactions` hook.
        let mut fork = self.fork();

        // We need to activate services before calling `create_patch()`; unlike all other blocks,
        // initial services are considered immediately active in the genesis block, i.e.,
        // their state should be included into `patch` created below.
        // TODO Unify block creation logic [ECR-3879]
        let errors = self.dispatcher.after_transactions(&mut fork);

        // If there was at least one error during the genesis block creation, the block shouldn't be
        // created at all.
        assert!(
            errors.is_empty(),
            "`after_transactions` failed for at least one service, errors: {:?}",
            &errors
        );

        let patch = self.dispatcher.commit_block(fork);
        self.merge(patch).unwrap();

        let (_, patch) = self.create_patch(
            ValidatorId::zero(),
            Height::zero(),
            &[],
            &mut BTreeMap::new(),
        );
        // On the other hand, we need to notify runtimes *after* the block has been created.
        // Otherwise, benign operations (e.g., calling `height()` on the core schema) will panic.
        self.dispatcher.notify_runtimes_about_commit(&patch);
        self.merge(patch).unwrap();

        log::info!(
            "GENESIS_BLOCK ====== hash={}",
            self.inner.last_hash().to_hex()
        );
    }

    /// Executes the given transactions from the pool. Collects the resulting changes
    /// from the current storage state and returns them with the hash of the resulting block.
    pub fn create_patch(
        &self,
        proposer_id: ValidatorId,
        height: Height,
        tx_hashes: &[Hash],
        tx_cache: &mut BTreeMap<Hash, Verified<AnyTx>>,
    ) -> (Hash, Patch) {
        self.create_patch_inner(self.fork(), proposer_id, height, tx_hashes, tx_cache)
    }

    /// Version of `create_patch` that supports user-provided fork. Used in tests.
    pub(crate) fn create_patch_inner(
        &self,
        mut fork: Fork,
        proposer_id: ValidatorId,
        height: Height,
        tx_hashes: &[Hash],
        tx_cache: &mut BTreeMap<Hash, Verified<AnyTx>>,
    ) -> (Hash, Patch) {
        // Skip execution for genesis block.
        if height > Height(0) {
            let errors = self.dispatcher.before_transactions(&mut fork);
            let mut call_errors = Schema::new(&fork).call_errors(height);
            for (location, error) in errors {
                call_errors.put(&location, error);
            }
        }

        // Save & execute transactions.
        for (index, hash) in (0..).zip(tx_hashes) {
            self.execute_transaction(*hash, height, index, &mut fork, tx_cache);
        }

        // During processing of the genesis block, this hook is already called in another method.
        if height > Height(0) {
            let errors = self.dispatcher.after_transactions(&mut fork);
            let mut call_errors = Schema::new(&fork).call_errors(height);
            for (location, error) in errors {
                call_errors.put(&location, error);
            }
        }

        let (patch, block) = self.create_block_header(fork, proposer_id, height, tx_hashes);
        log::trace!("Executing {:?}", block);

        // Calculate block hash.
        let block_hash = block.object_hash();
        // Update height.
        let fork = Fork::from(patch);
        let schema = Schema::new(&fork);
        schema.block_hashes_by_height().push(block_hash);
        // Save block.
        schema.blocks().put(&block_hash, block);
        (block_hash, fork.into_patch())
    }

    fn create_block_header(
        &self,
        fork: Fork,
        proposer_id: ValidatorId,
        height: Height,
        tx_hashes: &[Hash],
    ) -> (Patch, Block) {
        let prev_hash = self.inner.last_hash();

        let schema = Schema::new(&fork);
        let error_hash = schema.call_errors(height).object_hash();
        let tx_hash = schema.block_transactions(height).object_hash();
        let patch = fork.into_patch();
        let state_hash = SystemSchema::new(&patch).state_hash();

        let mut block = Block {
            height,
            tx_count: tx_hashes.len() as u32,
            prev_hash,
            tx_hash,
            state_hash,
            error_hash,
            additional_headers: AdditionalHeaders::new(),
        };

        block.add_header::<ProposerId>(proposer_id);

        (patch, block)
    }

    fn execute_transaction(
        &self,
        tx_hash: Hash,
        height: Height,
        index: u32,
        fork: &mut Fork,
        tx_cache: &mut BTreeMap<Hash, Verified<AnyTx>>,
    ) {
        let schema = Schema::new(&*fork);
        let transaction = get_transaction(&tx_hash, &schema.transactions(), &tx_cache)
            .unwrap_or_else(|| panic!("BUG: Cannot find transaction {:?} in database", tx_hash));
        fork.flush();

        let tx_result = self.dispatcher.execute(fork, tx_hash, index, &transaction);
        let mut schema = Schema::new(&*fork);

        if let Err(e) = tx_result {
            schema
                .call_errors(height)
                .put(&CallInBlock::transaction(index), e);
        }
        schema.commit_transaction(&tx_hash, height, transaction);
        tx_cache.remove(&tx_hash);
        let location = TxLocation::new(height, index);
        schema.transactions_locations().put(&tx_hash, location);
        fork.flush();
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
        let fork: Fork = patch.into();
        let mut schema = Schema::new(&fork);
        schema.precommits(&block_hash).extend(precommits);
        let txs_in_block = schema.last_block().tx_count;
        schema.update_transaction_count(u64::from(txs_in_block));

        let tx_hashes = tx_cache.keys().cloned().collect::<Vec<Hash>>();
        for tx_hash in tx_hashes {
            if let Some(tx) = tx_cache.remove(&tx_hash) {
                if !schema.transactions().contains(&tx_hash) {
                    schema.add_transaction_into_pool(tx);
                }
            }
        }

        let patch = self.dispatcher.commit_block_and_notify_runtimes(fork);
        self.merge(patch)?;
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

    /// Shuts down the service dispatcher enclosed by this blockchain. This must be
    /// the last operation performed on the blockchain.
    pub fn shutdown(&mut self) {
        self.dispatcher.shutdown();
    }
}

/// Returns transaction from the persistent pool. If transaction is not present in the pool, tries
/// to return it from the transactions cache.
#[doc(hidden)]
pub fn get_transaction<T: RawAccess>(
    hash: &Hash,
    txs: &MapIndex<T, Hash, Verified<AnyTx>>,
    tx_cache: &BTreeMap<Hash, Verified<AnyTx>>,
) -> Option<Verified<AnyTx>> {
    txs.get(&hash).or_else(|| tx_cache.get(&hash).cloned())
}

/// Check that transaction exists in the persistent pool or in the transaction cache.
#[doc(hidden)]
pub fn contains_transaction<T: RawAccess>(
    hash: &Hash,
    txs: &MapIndex<T, Hash, Verified<AnyTx>>,
    tx_cache: &BTreeMap<Hash, Verified<AnyTx>>,
) -> bool {
    txs.contains(&hash) || tx_cache.contains_key(&hash)
}
