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

use exonum_derive::{BinaryValue, ObjectHash};
use exonum_merkledb::{
    access::{Access, AccessExt, RawAccessMut},
    impl_binary_key_for_binary_value, Entry, KeySetIndex, ListIndex, MapIndex, ObjectHash,
    ProofEntry, ProofListIndex, ProofMapIndex,
};
use exonum_proto::ProtobufConvert;
use failure::format_err;

use std::fmt;

use super::{Block, BlockProof, ConsensusConfig, ExecutionError};
use crate::{
    crypto::{Hash, PublicKey},
    helpers::{Height, Round, ValidatorId},
    messages::{AnyTx, Connect, Message, Precommit, Verified},
    proto::{self, schema::blockchain as pb_blockchain},
    runtime::InstanceId,
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
    CALL_ERRORS => "call_errors";
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
#[derive(Debug, Clone, Copy, PartialEq)]
#[derive(Serialize, Deserialize)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "proto::TxLocation")]
pub struct TxLocation {
    /// Height of the block where the transaction was included.
    block_height: Height,
    /// Zero-based position of this transaction in the block.
    position_in_block: u64,
}

impl TxLocation {
    /// Creates a new transaction location.
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

/// Information schema for indexes maintained by the Exonum core logic.
///
/// Indexes defined by this schema are present in the blockchain regardless of
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
    #[doc(hidden)]
    pub fn new(access: T) -> Self {
        Self { access }
    }

    /// Returns a table that represents a map with a key-value pair of a
    /// transaction hash and raw transaction message.
    pub fn transactions(&self) -> MapIndex<T::Base, Hash, Verified<AnyTx>> {
        self.access.clone().get_map(TRANSACTIONS)
    }

    /// Returns a record of errors that occurred during execution of a particular block.
    ///
    /// This method can be used to retrieve a proof that execution of a certain transaction
    /// ended up with a particular status. Since the number of transaction in a block is
    /// mentioned in the block header, a proof of absence of an error for a transaction
    /// with a particular index means that it was executed successfully.
    ///
    /// Similarly, execution errors of the `before_transactions` / `after_transactions` hooks can be proven
    /// to external clients. Discerning successful execution from a non-existing service requires prior knowledge
    /// though.
    // TODO: Retain historic information about services [ECR-3922]
    pub fn call_errors(
        &self,
        block_height: Height,
    ) -> ProofMapIndex<T::Base, CallInBlock, ExecutionError> {
        self.access
            .clone()
            .get_proof_map((CALL_ERRORS, &block_height.0))
    }

    /// Returns the result of the execution for a transaction with the specified location.
    /// If the location does not correspond to a transaction, returns `None`.
    pub fn transaction_result(&self, location: TxLocation) -> Option<Result<(), ExecutionError>> {
        if self.block_transactions(location.block_height).len() <= location.position_in_block {
            return None;
        }

        let call_location = CallInBlock::transaction(location.position_in_block);
        let call_result = match self.call_errors(location.block_height).get(&call_location) {
            None => Ok(()),
            Some(e) => Err(e),
        };
        Some(call_result)
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
    #[doc(hidden)]
    pub fn transactions_pool_len_index(&self) -> Entry<T::Base, u64> {
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
    /// Panics if invoked before the genesis block was created, e.g. within
    /// `after_transactions` hook for genesis block.
    pub fn height(&self) -> Height {
        let len = self.block_hashes_by_height().len();
        assert!(
            len > 0,
            "An attempt to get the actual `height` during creating the genesis block."
        );
        Height(len - 1)
    }

    /// Returns the height of the block to be committed.
    ///
    /// Unlike `height`, this method never panics.
    pub fn next_height(&self) -> Height {
        let len = self.block_hashes_by_height().len();
        Height(len)
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

/// Location of an isolated call within a block.
///
/// Exonum isolates execution of the transactions included into the the block,
/// and `before_transactions` / `after_transactions` hooks that are executed for each active service.
/// If an isolated call ends with an error, all changes to the blockchain state made within a call
/// are rolled back.
///
/// `CallInBlock` objects are ordered in the same way the corresponding calls would be performed
/// within a block:
///
/// ```rust
/// # use exonum::blockchain::CallInBlock;
/// assert!(CallInBlock::before_transactions(3) < CallInBlock::transaction(0));
/// assert!(CallInBlock::transaction(0) < CallInBlock::transaction(1));
/// assert!(CallInBlock::transaction(1) < CallInBlock::after_transactions(0));
/// assert!(CallInBlock::after_transactions(0) < CallInBlock::after_transactions(1));
/// ```
///
/// # See also
///
/// Not to be confused with [`CallSite`], which provides information about a call in which
/// an error may occur. Since Exonum services may call each other's methods, `CallSite` is
/// richer than `CallInBlock`.
///
/// [`CallSite`]: ../runtime/error/struct.CallSite.html
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)] // builtin traits
#[derive(Serialize, Deserialize, BinaryValue, ObjectHash)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CallInBlock {
    /// Call of `before_transactions` hook in a service.
    BeforeTransactions {
        /// Numerical service identifier.
        id: InstanceId,
    },
    /// Call of a transaction within the block.
    Transaction {
        /// Zero-based transaction index.
        index: u64,
    },
    /// Call of `after_transactions` hook in a service.
    AfterTransactions {
        /// Numerical service identifier.
        id: InstanceId,
    },
}

impl ProtobufConvert for CallInBlock {
    type ProtoStruct = pb_blockchain::CallInBlock;

    fn to_pb(&self) -> Self::ProtoStruct {
        let mut pb = Self::ProtoStruct::new();
        match self {
            CallInBlock::BeforeTransactions { id } => pb.set_before_transactions(*id),
            CallInBlock::Transaction { index } => pb.set_transaction(*index),
            CallInBlock::AfterTransactions { id } => pb.set_after_transactions(*id),
        }
        pb
    }

    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, failure::Error> {
        if pb.has_before_transactions() {
            Ok(CallInBlock::BeforeTransactions {
                id: pb.get_before_transactions(),
            })
        } else if pb.has_transaction() {
            Ok(CallInBlock::Transaction {
                index: pb.get_transaction(),
            })
        } else if pb.has_after_transactions() {
            Ok(CallInBlock::AfterTransactions {
                id: pb.get_after_transactions(),
            })
        } else {
            Err(format_err!("Invalid location format"))
        }
    }
}

impl CallInBlock {
    /// Creates a location corresponding to a `before_transactions` call.
    pub fn before_transactions(id: InstanceId) -> Self {
        CallInBlock::BeforeTransactions { id }
    }

    /// Creates a location corresponding to a transaction.
    pub fn transaction(index: u64) -> Self {
        CallInBlock::Transaction { index }
    }

    /// Creates a location corresponding to a `after_transactions` call.
    pub fn after_transactions(id: InstanceId) -> Self {
        CallInBlock::AfterTransactions { id }
    }
}

impl_binary_key_for_binary_value!(CallInBlock);

impl fmt::Display for CallInBlock {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CallInBlock::BeforeTransactions { id } => write!(
                formatter,
                "`before_transactions` for service with ID {}",
                id
            ),
            CallInBlock::Transaction { index } => write!(formatter, "transaction #{}", index + 1),
            CallInBlock::AfterTransactions { id } => {
                write!(formatter, "`after_transactions` for service with ID {}", id)
            }
        }
    }
}

#[test]
fn location_json_serialization() {
    use serde_json::json;

    let location = CallInBlock::transaction(1);
    assert_eq!(
        serde_json::to_value(location).unwrap(),
        json!({ "type": "transaction", "index": 1 })
    );

    let location = CallInBlock::after_transactions(1_000);
    assert_eq!(
        serde_json::to_value(location).unwrap(),
        json!({ "type": "after_transactions", "id": 1_000 })
    );
}
