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
    BinaryKey, Entry, KeySetIndex, ListIndex, MapIndex, ObjectHash, ProofListIndex, ProofMapIndex,
};

use exonum_proto::ProtobufConvert;

use std::mem;

use super::{Block, BlockProof, ConsensusConfig, ExecutionStatus};
use crate::{
    crypto::{self, Hash, PublicKey},
    helpers::{Height, Round, ValidatorId},
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
    pub fn consensus_config_entry(&self) -> Entry<T::Base, ConsensusConfig> {
        self.access.clone().get_entry(CONSENSUS_CONFIG)
    }

    /// Returns the accessory `ProofMapIndex` for calculating
    /// patches in the DBView layer.
    ///
    /// The table calculates the "aggregation" of root hashes of individual
    /// service tables, in effect summing the state of various entities,
    /// scattered across distinct services and their tables. Sum is performed by
    /// means of computing the root hash of this table.
    ///
    /// - Table **key** contains normalized coordinates of an index.
    /// - Table **value** contains a root hash of the index, which contributes
    /// to the `state_hash` of the resulting block.
    pub fn state_hash_aggregator(&self) -> ProofMapIndex<T::Base, IndexCoordinates, Hash> {
        self.access.clone().get_proof_map(STATE_HASH_AGGREGATOR)
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

    /// Returns the `state_hash` table for core tables.
    pub fn state_hash(&self) -> Vec<Hash> {
        vec![
            self.consensus_config_entry().object_hash(),
            self.transaction_results().object_hash(),
        ]
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

/// Describes the origin of the information schema.
///
/// A schema origin is a convenient wrapper over a two first parameters of an
/// [`IndexCoordinates`](struct.IndexCoordinates.html) to simple calculation of coordinates of the specific index.
///
/// # Examples
///
/// ```
/// # use exonum::blockchain::SchemaOrigin;
/// // Compute coordinate for the first index of runtime schema with ID 0.
/// let runtime_coordinate = SchemaOrigin::Runtime(0).coordinate_for(0);
/// // Compute coordinate for the first index of service schema with instance ID 0.
/// let schema_coordinate = SchemaOrigin::Service(0).coordinate_for(0);
/// // Note that the `origin_label` of these coordinates are different
/// // but `local_schema_id` are same.
/// assert_ne!(
///     runtime_coordinate.origin_label,
///     schema_coordinate.origin_label
/// );
/// assert_eq!(
///     runtime_coordinate.local_schema_id,
///     schema_coordinate.local_schema_id
/// );
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SchemaOrigin {
    /// This is a core schema.
    Core,
    /// This is a dispatcher schema.
    Dispatcher,
    /// Schema belongs to the runtime with the specified ID.
    Runtime(u32),
    /// This is a service schema with the specified instance ID.
    Service(InstanceId),
}

impl SchemaOrigin {
    /// Computes coordinates for a given schema index.
    pub fn coordinate_for(self, index_id: u16) -> IndexCoordinates {
        IndexCoordinates::new(self, index_id)
    }

    /// Returns the corresponding origin label.
    fn origin_label(self) -> OriginLabel {
        match self {
            SchemaOrigin::Core => OriginLabel::Core,
            SchemaOrigin::Dispatcher => OriginLabel::Dispatcher,
            SchemaOrigin::Runtime { .. } => OriginLabel::Runtime,
            SchemaOrigin::Service { .. } => OriginLabel::Service,
        }
    }

    /// Returns the corresponding schema ID.
    fn local_schema_id(self) -> u32 {
        match self {
            SchemaOrigin::Service(instance_id) => instance_id,
            SchemaOrigin::Runtime(runtime_id) => runtime_id,
            SchemaOrigin::Core => 0,
            SchemaOrigin::Dispatcher => 1,
        }
    }
}

/// Label for the corresponding schema origin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(u16)]
pub enum OriginLabel {
    /// Origin label for Core schema.
    Core = 0,
    /// Origin label for Dispatcher schema.
    Dispatcher = 1,
    /// Origin label for runtime schemas.
    Runtime = 2,
    /// Origin label for service schemas.
    Service = 3,
}

/// Normalized coordinates of the index in the [`state_hash_aggregator`][state_hash_aggregator] table.
///
/// This coordinate is used to map the index to its contribution to the blockchain state hash.
/// Each index has its own unique coordinates.
///
/// [See also.][SchemaOrigin]
///
/// [state_hash_aggregator]: struct.Schema.html#method.state_hash_aggregator
/// [SchemaOrigin]: enum.SchemaOrigin.html
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct IndexCoordinates {
    /// Determines which category of an information schemas an index belongs to.
    pub origin_label: u16,
    /// Identifier of the schema to which the index belongs, should be unique in the corresponding
    /// origin category.
    pub local_schema_id: u32,
    /// Index identifier in the corresponding information schema.
    pub index_id: u16,
}

impl IndexCoordinates {
    /// Creates index coordinates for the index with the specified schema origin
    /// and index identifier.
    pub fn new(schema_origin: SchemaOrigin, index_id: u16) -> Self {
        Self {
            origin_label: schema_origin.origin_label() as u16,
            local_schema_id: schema_origin.local_schema_id(),
            index_id,
        }
    }

    /// For the given schema origin, returns a list of the index coordinates that match the
    /// corresponding hashes of the indices.
    pub fn locate(
        schema_origin: SchemaOrigin,
        object_hashes: impl IntoIterator<Item = Hash>,
    ) -> impl IntoIterator<Item = (IndexCoordinates, Hash)> {
        object_hashes
            .into_iter()
            .enumerate()
            .map(move |(id, hash)| (schema_origin.coordinate_for(id as u16), hash))
    }

    /// Returns a schema origin for this index.
    pub fn schema_origin(self) -> SchemaOrigin {
        match self.origin_label {
            0 => SchemaOrigin::Core,
            2 => SchemaOrigin::Runtime(self.local_schema_id),
            3 => SchemaOrigin::Service(self.local_schema_id),
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
        pos += self.origin_label.write(&mut buffer[pos..]);
        pos += self.local_schema_id.write(&mut buffer[pos..]);
        pos += self.index_id.write(&mut buffer[pos..]);
        pos
    }

    fn read(buffer: &[u8]) -> Self::Owned {
        let origin_label = u16::read(&buffer[0..2]);
        let local_schema_id = u32::read(&buffer[2..6]);
        let index_id = u16::read(&buffer[6..8]);
        Self {
            origin_label,
            local_schema_id,
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
fn index_coordinates_binary_key_round_trip() {
    let schema_origins = vec![
        (SchemaOrigin::Runtime(0), 0),
        (SchemaOrigin::Runtime(0), 5),
        (SchemaOrigin::Runtime(1), 0),
        (SchemaOrigin::Runtime(1), 2),
        (SchemaOrigin::Service(2), 0),
        (SchemaOrigin::Service(2), 1),
        (SchemaOrigin::Service(0), 0),
        (SchemaOrigin::Service(0), 1),
    ];

    for (schema_origin, index_id) in schema_origins {
        let coordinate = IndexCoordinates::new(schema_origin, index_id);
        let mut buf = vec![0; coordinate.size()];
        coordinate.write(&mut buf);

        let coordinate2 = IndexCoordinates::read(&buf);
        assert_eq!(coordinate, coordinate2);
        assert_eq!(coordinate2.schema_origin(), schema_origin);
    }
}
