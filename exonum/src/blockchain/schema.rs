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
    BinaryKey, Entry, IndexAccess, KeySetIndex, ListIndex, MapIndex, ObjectHash, ProofListIndex,
    ProofMapIndex,
};

use exonum_proto::ProtobufConvert;

use std::mem;

use super::{Block, BlockProof, ConsensusConfig, ExecutionStatus};
use crate::{
    crypto::{self, Hash, PublicKey},
    helpers::{Height, Round},
    messages::{AnyTx, Connect, Message, Precommit, Verified},
    proto,
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
    TRANSACTION_RESULTS => "transaction_results";
    TRANSACTIONS_LEN => "transactions_len";
    TRANSACTIONS_POOL => "transactions_pool";
    TRANSACTIONS_POOL_LEN => "transactions_pool_len";
    TRANSACTIONS_LOCATIONS => "transactions_locations";
    BLOCKS => "blocks";
    BLOCK_HASHES_BY_HEIGHT => "block_hashes_by_height";
    BLOCK_TRANSACTIONS => "block_transactions";
    PRECOMMITS => "precommits";
    STATE_HASH_AGGREGATOR => "state_hash_aggregator";
    PEERS_CACHE => "peers_cache";
    CONSENSUS_MESSAGES_CACHE => "consensus_messages_cache";
    CONSENSUS_ROUND => "consensus_round";
    CONSENSUS_CONFIG => "consensus.config";
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
#[derive(Debug)]
pub struct Schema<T> {
    access: T,
}

impl<T> Schema<T>
where
    T: IndexAccess,
{
    /// Constructs information schema for the given `snapshot`.
    pub fn new(access: T) -> Self {
        Self { access }
    }

    /// Returns a table that represents a map with a key-value pair of a
    /// transaction hash and raw transaction message.
    pub fn transactions(&self) -> MapIndex<T, Hash, Verified<AnyTx>> {
        MapIndex::new(TRANSACTIONS, self.access.clone())
    }

    /// Returns a table that represents a map with a key-value pair of a transaction
    /// hash and execution result.
    ///
    /// This method can be used to retrieve a proof that a certain transaction
    /// result is present in the blockchain.
    pub fn transaction_results(&self) -> ProofMapIndex<T, Hash, ExecutionStatus> {
        ProofMapIndex::new(TRANSACTION_RESULTS, self.access.clone())
    }

    /// Returns an entry that represents a count of committed transactions in the blockchain.
    pub(crate) fn transactions_len_index(&self) -> Entry<T, u64> {
        Entry::new(TRANSACTIONS_LEN, self.access.clone())
    }

    /// Returns the number of transactions in the blockchain.
    pub fn transactions_len(&self) -> u64 {
        // TODO: Change a count of tx logic after replacement storage to MerkleDB. ECR-3087
        let pool = self.transactions_len_index();
        pool.get().unwrap_or(0)
    }

    /// Returns a table that represents a set of uncommitted transactions hashes.
    pub fn transactions_pool(&self) -> KeySetIndex<T, Hash> {
        KeySetIndex::new(TRANSACTIONS_POOL, self.access.clone())
    }

    /// Returns an entry that represents count of uncommitted transactions.
    pub(crate) fn transactions_pool_len_index(&self) -> Entry<T, u64> {
        Entry::new(TRANSACTIONS_POOL_LEN, self.access.clone())
    }

    /// Returns the number of transactions in the pool.
    pub fn transactions_pool_len(&self) -> u64 {
        let pool = self.transactions_pool_len_index();
        pool.get().unwrap_or(0)
    }

    /// Returns a table that keeps the block height and transaction position inside the block for every
    /// transaction hash.
    pub fn transactions_locations(&self) -> MapIndex<T, Hash, TxLocation> {
        MapIndex::new(TRANSACTIONS_LOCATIONS, self.access.clone())
    }

    /// Returns a table that stores a block object for every block height.
    pub fn blocks(&self) -> MapIndex<T, Hash, Block> {
        MapIndex::new(BLOCKS, self.access.clone())
    }

    /// Returns a table that keeps block hashes for corresponding block heights.
    pub fn block_hashes_by_height(&self) -> ListIndex<T, Hash> {
        ListIndex::new(BLOCK_HASHES_BY_HEIGHT, self.access.clone())
    }

    /// Returns a table that keeps a list of transactions for each block.
    pub fn block_transactions(&self, height: Height) -> ProofListIndex<T, Hash> {
        let height: u64 = height.into();
        ProofListIndex::new_in_family(BLOCK_TRANSACTIONS, &height, self.access.clone())
    }

    /// Returns a table that keeps a list of precommits for the block with the given hash.
    pub fn precommits(&self, hash: &Hash) -> ListIndex<T, Verified<Precommit>> {
        ListIndex::new_in_family(PRECOMMITS, hash, self.access.clone())
    }

    /// Returns an actual consensus configuration entry.
    pub fn consensus_config_entry(&self) -> Entry<T, ConsensusConfig> {
        Entry::new(CONSENSUS_CONFIG, self.access.clone())
    }

    /// Returns the accessory `ProofMapIndex` for calculating
    /// patches in the DBView layer.
    ///
    /// The table calculates the "aggregation" of root hashes of individual
    /// service tables, in effect summing the state of various entities,
    /// scattered across distinct services and their tables. Sum is performed by
    /// means of computing the root hash of this table.
    ///
    /// - Table **key** is  normalized coordinates of a service.
    /// - Table **value** is the root hash of a service table, which contributes
    /// to the `state_hash` of the resulting block.
    ///
    /// Core tables participate in the resulting state_hash with `CORE_ID`
    /// service_id. Their vector is returned by the `core_state_hash` method.
    pub fn state_hash_aggregator(&self) -> ProofMapIndex<T, IndexCoordinates, Hash> {
        ProofMapIndex::new(STATE_HASH_AGGREGATOR, self.access.clone())
    }

    /// Returns peers that have to be recovered in case of process restart
    /// after abnormal termination.
    pub(crate) fn peers_cache(&self) -> MapIndex<T, PublicKey, Verified<Connect>> {
        MapIndex::new(PEERS_CACHE, self.access.clone())
    }

    /// Returns consensus messages that have to be recovered in case of process restart
    /// after abnormal termination.
    pub(crate) fn consensus_messages_cache(&self) -> ListIndex<T, Message> {
        ListIndex::new(CONSENSUS_MESSAGES_CACHE, self.access.clone())
    }

    /// Returns the saved value of the consensus round. Returns the first round
    /// if it has not been saved.
    pub(crate) fn consensus_round(&self) -> Round {
        Entry::new(CONSENSUS_ROUND, self.access.clone())
            .get()
            .unwrap_or_else(Round::first)
    }

    /// Returns the block hash for the given height.
    pub fn block_hash_by_height(&self, height: Height) -> Option<Hash> {
        self.block_hashes_by_height().get(height.into())
    }

    /// Returns the block for the given height with the proof of its inclusion.
    pub fn block_and_precommits(&self, height: Height) -> Option<BlockProof> {
        let block_hash = match self.block_hash_by_height(height) {
            None => return None,
            Some(block_hash) => block_hash,
        };
        let block = self.blocks().get(&block_hash).unwrap();
        let precommits_table = self.precommits(&block_hash);
        let precommits = precommits_table.iter().collect();
        let res = BlockProof { block, precommits };
        Some(res)
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

    /// Returns the `state_hash` table for core tables.
    pub fn state_hash(&self) -> Vec<Hash> {
        vec![
            self.consensus_config_entry().object_hash(),
            self.transaction_results().object_hash(),
        ]
    }

    /// Saves the given consensus round value into the storage.
    pub(crate) fn set_consensus_round(&mut self, round: Round) {
        let mut entry: Entry<T, _> = Entry::new(CONSENSUS_ROUND, self.access.clone());
        entry.set(round);
    }

    /// Adds transaction into the persistent pool.
    /// This method increment `transactions_pool_len_index`,
    /// be sure to decrement it when transaction committed.
    #[doc(hidden)]
    pub fn add_transaction_into_pool(&mut self, tx: Verified<AnyTx>) {
        self.transactions_pool().insert(tx.object_hash());
        let x = self.transactions_pool_len_index().get().unwrap_or(0);
        self.transactions_pool_len_index().set(x + 1);
        self.transactions().put(&tx.object_hash(), tx);
    }

    /// Changes the transaction status from `in_pool`, to `committed`.
    pub(crate) fn commit_transaction(&mut self, hash: &Hash, tx: Verified<AnyTx>) {
        if !self.transactions().contains(hash) {
            self.transactions().put(hash, tx)
        }

        if self.transactions_pool().contains(hash) {
            self.transactions_pool().remove(hash);
            let txs_pool_len = self.transactions_pool_len_index().get().unwrap();
            self.transactions_pool_len_index().set(txs_pool_len - 1);
        }
    }

    /// Updates transaction count of the blockchain.
    pub fn update_transaction_count(&mut self, count: u64) {
        let mut len_index = self.transactions_len_index();
        let new_len = len_index.get().unwrap_or(0) + count;
        len_index.set(new_len);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IndexOwner {
    /// This index is part of core schema.
    Core,
    /// This index is part of dispatcher schema.
    Dispatcher,
    /// This index is a part of runtime schema.
    Runtime(u32),
    /// This index is a part of some service schema.
    Service(InstanceId),
}

impl IndexOwner {
    /// Creates index coordinate for the current owner.
    pub fn coordinate_for(self, index_id: u16) -> IndexCoordinates {
        IndexCoordinates::new(self, index_id)
    }

    /// Returns the corresponding tag.
    fn tag(self) -> IndexTag {
        match self {
            IndexOwner::Core => IndexTag::Core,
            IndexOwner::Dispatcher => IndexTag::Dispatcher,
            IndexOwner::Runtime { .. } => IndexTag::Runtime,
            IndexOwner::Service { .. } => IndexTag::Service,
        }
    }

    /// Returns the corresponding group id.
    fn group_id(self) -> u32 {
        match self {
            IndexOwner::Service(instance_id) => instance_id,
            IndexOwner::Runtime(runtime_id) => runtime_id,
            IndexOwner::Core | IndexOwner::Dispatcher => 0,
        }
    }
}

/// Binary value for the corresponding index owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(u16)]
enum IndexTag {
    Core = 0,
    Dispatcher = 1,
    Runtime = 2,
    Service = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct IndexCoordinates {
    tag: u16,
    group_id: u32,
    index_id: u16,
}

impl IndexCoordinates {
    /// Creates index coordinated for the index with the specified owner and identifier.
    pub fn new(owner: IndexOwner, index_id: u16) -> Self {
        Self {
            tag: owner.tag() as u16,
            group_id: owner.group_id(),
            index_id,
        }
    }

    pub fn locate(
        owner: IndexOwner,
        object_hashes: impl IntoIterator<Item = Hash>,
    ) -> impl IntoIterator<Item = (IndexCoordinates, Hash)> {
        object_hashes
            .into_iter()
            .enumerate()
            .map(move |(id, hash)| (owner.coordinate_for(id as u16), hash))
    }

    pub fn owner(self) -> IndexOwner {
        match self.tag {
            0 => IndexOwner::Core,
            1 => IndexOwner::Dispatcher,
            2 => IndexOwner::Runtime(self.group_id),
            3 => IndexOwner::Service(self.group_id),
            other => panic!("Unknown index owner: {}!", other),
        }
    }
}

impl BinaryKey for IndexCoordinates {
    fn size(&self) -> usize {
        mem::size_of_val(self)
    }

    fn write(&self, buffer: &mut [u8]) -> usize {
        let mut pos = 0;
        pos += self.tag.write(&mut buffer[pos..]);
        pos += self.group_id.write(&mut buffer[pos..]);
        pos += self.index_id.write(&mut buffer[pos..]);
        pos
    }

    fn read(buffer: &[u8]) -> Self::Owned {
        let tag = u16::read(&buffer[0..2]);
        let group_id = u32::read(&buffer[2..6]);
        let index_id = u16::read(&buffer[6..8]);
        Self {
            tag,
            group_id,
            index_id,
        }
    }
}

impl ObjectHash for IndexCoordinates {
    fn object_hash(&self) -> Hash {
        let mut bytes = vec![0; self.size()];
        self.write(&mut bytes);
        crypto::hash(&bytes)
    }
}

#[test]
fn test_index_coordinates_binary_key_round_trip() {
    let index_owners = vec![
        (IndexOwner::Dispatcher, 0),
        (IndexOwner::Dispatcher, 1),
        (IndexOwner::Runtime(0), 0),
        (IndexOwner::Runtime(0), 5),
        (IndexOwner::Runtime(1), 0),
        (IndexOwner::Runtime(1), 2),
        (IndexOwner::Service(2), 0),
        (IndexOwner::Service(2), 1),
        (IndexOwner::Service(0), 0),
        (IndexOwner::Service(0), 1),
    ];

    for (owner, id) in index_owners {
        let coordinate = IndexCoordinates::new(owner, id);
        let mut buf = vec![0; coordinate.size()];
        coordinate.write(&mut buf);

        let coordinate2 = IndexCoordinates::read(&buf);
        assert_eq!(coordinate, coordinate2);
        assert_eq!(coordinate2.owner(), owner);
    }
}
