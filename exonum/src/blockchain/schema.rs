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
// WITHOUT WARRANTIES OR CONDITIONS OF ANY owner, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use exonum_merkledb::{
    access::{Access, AccessExt, RawAccessMut},
    Entry, KeySetIndex, ListIndex, MapIndex, ObjectHash, ProofEntry, ProofListIndex, ProofMapIndex,
};

use exonum_proto::ProtobufConvert;

use super::{Block, BlockProof, ConsensusConfig, ExecutionStatus};
use crate::{
    crypto::{Hash, PublicKey},
    helpers::{Height, Round, ValidatorId},
    messages::{AnyTx, Connect, Message, Precommit, Verified},
    proto,
};

/// Defines `&str` constants with given name and value.
macro_rules! define_names {
    (
        $(
            $name:ident => $value:expr;
        )+
    ) => (
        $(const $name: &str = concat!("core.", $value);)*
    )
}

define_names!(
    TRANSACTIONS => "transactions";
    TRANSACTION_RESULTS => "transaction_results";
    TRANSACTIONS_LEN => "transactions_len";
    TRANSACTIONS_POOL => "transactions_pool";
    TRANSACTIONS_POOL_LEN => "transactions_pool_len";
    TRANSACTIONS_LOCATIONS => "transactions_locations";
    BLOCKS => "blocks";
    BLOCK_HASHES_BY_HEIGHT => "block_hashes_by_height";
    BLOCK_TRANSACTIONS => "block_transactions";
    PRECOMMITS => "precommits";
    PEERS_CACHE => "peers_cache";
    CONSENSUS_MESSAGES_CACHE => "consensus_messages_cache";
    CONSENSUS_ROUND => "consensus_round";
    CONSENSUS_CONFIG => "consensus_config";
);

/// Transaction location in a block.
/// The given entity defines the block where the transaction was
/// included and the position of this transaction in that block.
#[derive(Debug, Serialize, Deserialize, PartialEq, ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "proto::TxLocation")]
pub struct TxLocation {
    /// Height of the block where the transaction was included.
    block_height: Height,
    /// Zero-based position of this transaction in the block.
    position_in_block: u64,
}

impl TxLocation {
    /// New tx_location
    pub fn new(block_height: Height, position_in_block: u64) -> Self {
        Self {
            block_height,
            position_in_block,
        }
    }

    /// Height of the block where the transaction was included.
    pub fn block_height(&self) -> Height {
        self.block_height
    }
    /// Zero-based position of this transaction in the block.
    pub fn position_in_block(&self) -> u64 {
        self.position_in_block
    }
}

/// Information schema for indices maintained by the Exonum core logic.
///
/// Indices defined by this schema are present in the blockchain regardless of
/// the deployed services and store general-purpose information, such as
/// committed transactions.
#[derive(Debug, Clone, Copy)]
pub struct Schema<T> {
    // For performance reasons, we don't use the field-per-index schema pattern.
    // Indeed, the core schema has many indexes, most of which are never accessed
    // for any particular `Schema` instance.
    access: T,
}

impl<T: Access> Schema<T> {
    /// Constructs information schema based on the given `access`.
    pub(crate) fn new(access: T) -> Self {
        Self { access }
    }

    /// Returns a table that represents a map with a key-value pair of a
    /// transaction hash and raw transaction message.
    pub fn transactions(&self) -> MapIndex<T::Base, Hash, Verified<AnyTx>> {
        self.access.clone().get_map(TRANSACTIONS)
    }

    /// Returns a table that represents a map with a key-value pair of a transaction
    /// hash and execution result.
    ///
    /// This method can be used to retrieve a proof that a certain transaction
    /// result is present in the blockchain.
    pub fn transaction_results(&self) -> ProofMapIndex<T::Base, Hash, ExecutionStatus> {
        self.access.clone().get_proof_map(TRANSACTION_RESULTS)
    }

    /// Returns an entry that represents a count of committed transactions in the blockchain.
    pub(crate) fn transactions_len_index(&self) -> Entry<T::Base, u64> {
        self.access.clone().get_entry(TRANSACTIONS_LEN)
    }

    /// Returns the number of transactions in the blockchain.
    pub fn transactions_len(&self) -> u64 {
        // TODO: Change a count of tx logic after replacement storage to MerkleDB. ECR-3087
        let pool = self.transactions_len_index();
        pool.get().unwrap_or(0)
    }

    /// Returns a table that represents a set of uncommitted transactions hashes.
    pub fn transactions_pool(&self) -> KeySetIndex<T::Base, Hash> {
        self.access.clone().get_key_set(TRANSACTIONS_POOL)
    }

    /// Returns an entry that represents count of uncommitted transactions.
    pub(crate) fn transactions_pool_len_index(&self) -> Entry<T::Base, u64> {
        self.access.clone().get_entry(TRANSACTIONS_POOL_LEN)
    }

    /// Returns the number of transactions in the pool.
    pub fn transactions_pool_len(&self) -> u64 {
        let pool = self.transactions_pool_len_index();
        pool.get().unwrap_or(0)
    }

    /// Returns a table that keeps the block height and transaction position inside the block for every
    /// transaction hash.
    pub fn transactions_locations(&self) -> MapIndex<T::Base, Hash, TxLocation> {
        self.access.clone().get_map(TRANSACTIONS_LOCATIONS)
    }

    /// Returns a table that stores a block object for every block height.
    pub fn blocks(&self) -> MapIndex<T::Base, Hash, Block> {
        self.access.clone().get_map(BLOCKS)
    }

    /// Returns a table that keeps block hashes for corresponding block heights.
    pub fn block_hashes_by_height(&self) -> ListIndex<T::Base, Hash> {
        self.access.clone().get_list(BLOCK_HASHES_BY_HEIGHT)
    }

    /// Returns a table that keeps a list of transactions for each block.
    pub fn block_transactions(&self, height: Height) -> ProofListIndex<T::Base, Hash> {
        let height: u64 = height.into();
        self.access
            .clone()
            .get_proof_list((BLOCK_TRANSACTIONS, &height))
    }

    /// Returns a table that keeps a list of precommits for the block with the given hash.
    pub fn precommits(&self, hash: &Hash) -> ListIndex<T::Base, Verified<Precommit>> {
        self.access.clone().get_list((PRECOMMITS, hash))
    }

    /// Returns an actual consensus configuration entry.
    pub fn consensus_config_entry(&self) -> ProofEntry<T::Base, ConsensusConfig> {
        self.access.clone().get_proof_entry(CONSENSUS_CONFIG)
    }

    /// Returns peers that have to be recovered in case of process restart
    /// after abnormal termination.
    pub(crate) fn peers_cache(&self) -> MapIndex<T::Base, PublicKey, Verified<Connect>> {
        self.access.clone().get_map(PEERS_CACHE)
    }

    /// Returns consensus messages that have to be recovered in case of process restart
    /// after abnormal termination.
    pub(crate) fn consensus_messages_cache(&self) -> ListIndex<T::Base, Message> {
        self.access.clone().get_list(CONSENSUS_MESSAGES_CACHE)
    }

    /// Returns the saved value of the consensus round. Returns the first round
    /// if it has not been saved.
    pub(crate) fn consensus_round(&self) -> Round {
        self.access
            .clone()
            .get_entry(CONSENSUS_ROUND)
            .get()
            .unwrap_or_else(Round::first)
    }

    /// Returns the block hash for the given height.
    pub fn block_hash_by_height(&self, height: Height) -> Option<Hash> {
        self.block_hashes_by_height().get(height.into())
    }

    /// Returns the block for the given height with the proof of its inclusion.
    pub fn block_and_precommits(&self, height: Height) -> Option<BlockProof> {
        let block_hash = self.block_hash_by_height(height)?;
        let block = self.blocks().get(&block_hash).unwrap();
        let precommits = self.precommits(&block_hash).iter().collect();
        Some(BlockProof { block, precommits })
    }

    /// Returns the latest committed block.
    ///
    /// # Panics
    ///
    /// Panics if the "genesis block" was not created.
    pub fn last_block(&self) -> Block {
        let hash = self
            .block_hashes_by_height()
            .last()
            .expect("An attempt to get the `last_block` during creating the genesis block.");
        self.blocks().get(&hash).unwrap()
    }

    /// Returns the height of the latest committed block.
    ///
    /// # Panics
    ///
    /// Panics if the "genesis block" was not created.
    pub fn height(&self) -> Height {
        let len = self.block_hashes_by_height().len();
        assert!(
            len > 0,
            "An attempt to get the actual `height` during creating the genesis block."
        );
        Height(len - 1)
    }

    /// Returns an actual consensus configuration of the blockchain.
    ///
    /// # Panics
    ///
    /// Panics if the "genesis block" was not created.
    pub fn consensus_config(&self) -> ConsensusConfig {
        self.consensus_config_entry()
            .get()
            .expect("Consensus configuration is absent")
    }

    /// Attempts to find a `ValidatorId` by the provided service public key.
    pub fn validator_id(&self, service_public_key: PublicKey) -> Option<ValidatorId> {
        self.consensus_config()
            .find_validator(|validator_keys| service_public_key == validator_keys.service_key)
    }
}

impl<T: Access> Schema<T>
where
    T::Base: RawAccessMut,
{
    /// Saves the given consensus round value into the storage.
    pub(crate) fn set_consensus_round(&mut self, round: Round) {
        self.access.clone().get_entry(CONSENSUS_ROUND).set(round);
    }

    /// Adds a transaction into the persistent pool. The caller must ensure that the transaction
    /// is not already in the pool.
    ///
    /// This method increments the number of transactions in the pool,
    /// be sure to decrement it when the transaction committed.
    #[doc(hidden)]
    pub fn add_transaction_into_pool(&mut self, tx: Verified<AnyTx>) {
        self.transactions_pool().insert(tx.object_hash());
        let x = self.transactions_pool_len_index().get().unwrap_or(0);
        self.transactions_pool_len_index().set(x + 1);
        self.transactions().put(&tx.object_hash(), tx);
    }

    /// Changes the transaction status from `in_pool`, to `committed`.
    pub(crate) fn commit_transaction(&mut self, hash: &Hash, height: Height, tx: Verified<AnyTx>) {
        if !self.transactions().contains(hash) {
            self.transactions().put(hash, tx)
        }

        if self.transactions_pool().contains(hash) {
            self.transactions_pool().remove(hash);
            let txs_pool_len = self.transactions_pool_len_index().get().unwrap();
            self.transactions_pool_len_index().set(txs_pool_len - 1);
        }

        self.block_transactions(height).push(*hash);
    }

    /// Updates transaction count of the blockchain.
    pub(crate) fn update_transaction_count(&mut self, count: u64) {
        let mut len_index = self.transactions_len_index();
        let new_len = len_index.get().unwrap_or(0) + count;
        len_index.set(new_len);
    }
}
