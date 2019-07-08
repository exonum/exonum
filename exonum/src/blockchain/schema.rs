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

use exonum_merkledb::{
    BinaryKey, Entry, IndexAccess, KeySetIndex, ListIndex, MapIndex, MapProof, ObjectHash,
    ProofListIndex, ProofMapIndex,
};

use std::mem;

use crate::{
    crypto::{Hash, PublicKey},
    helpers::{Height, Round},
    messages::{AnyTx, Connect, Message, Precommit, ServiceInstanceId, Signed},
    proto,
};

use super::{config::StoredConfiguration, Block, BlockProof, Blockchain, TransactionResult};

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
    CONFIGS => "configs";
    CONFIGS_ACTUAL_FROM => "configs_actual_from";
    STATE_HASH_AGGREGATOR => "state_hash_aggregator";
    PEERS_CACHE => "peers_cache";
    CONSENSUS_MESSAGES_CACHE => "consensus_messages_cache";
    CONSENSUS_ROUND => "consensus_round";
);

/// Configuration index.
#[derive(Debug, Serialize, Deserialize, ProtobufConvert)]
#[exonum(pb = "proto::ConfigReference", crate = "crate")]
pub struct ConfigReference {
    /// Height since which this configuration becomes actual.
    actual_from: Height,
    /// Hash of the configuration contents that serialized as raw bytes vec.
    cfg_hash: Hash,
}

impl ConfigReference {
    /// New ConfigReference
    pub fn new(actual_from: Height, cfg_hash: &Hash) -> Self {
        Self {
            actual_from,
            cfg_hash: *cfg_hash,
        }
    }

    /// Height since which this configuration becomes actual.
    pub fn actual_from(&self) -> Height {
        self.actual_from
    }

    /// Hash of the configuration contents that serialized as raw bytes vec.
    pub fn cfg_hash(&self) -> &Hash {
        &self.cfg_hash
    }
}

/// Transaction location in a block.
/// The given entity defines the block where the transaction was
/// included and the position of this transaction in that block.
#[derive(Debug, Serialize, Deserialize, PartialEq, ProtobufConvert)]
#[exonum(pb = "proto::TxLocation", crate = "crate")]
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
    pub fn transactions(&self) -> MapIndex<T, Hash, Signed<AnyTx>> {
        MapIndex::new(TRANSACTIONS, self.access.clone())
    }

    /// Returns a table that represents a map with a key-value pair of a transaction
    /// hash and execution result.
    ///
    /// This method can be used to retrieve a proof that a certain transaction
    /// result is present in the blockchain.
    pub fn transaction_results(&self) -> ProofMapIndex<T, Hash, TransactionResult> {
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
    pub fn precommits(&self, hash: &Hash) -> ListIndex<T, Signed<Precommit>> {
        ListIndex::new_in_family(PRECOMMITS, hash, self.access.clone())
    }

    /// Returns a table that represents a map with a key-value pair of a
    /// configuration hash and contents.
    pub fn configs(&self) -> ProofMapIndex<T, Hash, StoredConfiguration> {
        // configs patricia merkle tree <block height> json
        ProofMapIndex::new(CONFIGS, self.access.clone())
    }

    /// Returns an auxiliary table that keeps hash references to configurations in
    /// the increasing order of their `actual_from` height.
    pub fn configs_actual_from(&self) -> ListIndex<T, ConfigReference> {
        ListIndex::new(CONFIGS_ACTUAL_FROM, self.access.clone())
    }

    /// Returns the accessory `ProofMapIndex` for calculating
    /// patches in the DBView layer.
    ///
    /// The table calculates the "aggregation" of root hashes of individual
    /// service tables, in effect summing the state of various entities,
    /// scattered across distinct services and their tables. Sum is performed by
    /// means of computing the root hash of this table.
    ///
    /// - Table **key** is 32 bytes of normalized coordinates of a service
    /// table, as returned by the `service_table_unique_key` helper function.
    /// - Table **value** is the root hash of a service table, which contributes
    /// to the `state_hash` of the resulting block.
    ///
    /// Core tables participate in the resulting state_hash with `CORE_ID`
    /// service_id. Their vector is returned by the `core_state_hash` method.
    pub fn state_hash_aggregator(&self) -> ProofMapIndex<T, Hash, Hash> {
        ProofMapIndex::new(STATE_HASH_AGGREGATOR, self.access.clone())
    }

    /// Returns peers that have to be recovered in case of process restart
    /// after abnormal termination.
    pub(crate) fn peers_cache(&self) -> MapIndex<T, PublicKey, Signed<Connect>> {
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

    /// Returns the configuration for the latest height of the blockchain.
    ///
    /// # Panics
    ///
    /// Panics if the "genesis block" was not created.
    pub fn actual_configuration(&self) -> StoredConfiguration {
        let next_height = self.next_height();
        let res = self.configuration_by_height(next_height);
        trace!("Retrieved actual_config: {:?}", res);
        res
    }

    /// Returns the nearest following configuration which will be applied after
    /// the current one, if it exists.
    pub fn following_configuration(&self) -> Option<StoredConfiguration> {
        let next_height = self.next_height();
        let idx = self.find_configurations_index_by_height(next_height);
        match self.configs_actual_from().get(idx + 1) {
            Some(cfg_ref) => {
                let cfg_hash = cfg_ref.cfg_hash();
                let cfg = self.configuration_by_hash(cfg_hash).unwrap_or_else(|| {
                    panic!("Config with hash {:?} is absent in configs table", cfg_hash)
                });
                Some(cfg)
            }
            None => None,
        }
    }

    /// Returns the previous configuration if it exists.
    pub fn previous_configuration(&self) -> Option<StoredConfiguration> {
        let next_height = self.next_height();
        let idx = self.find_configurations_index_by_height(next_height);
        if idx > 0 {
            let cfg_ref = self
                .configs_actual_from()
                .get(idx - 1)
                .unwrap_or_else(|| panic!("Configuration at index {} not found", idx));
            let cfg_hash = cfg_ref.cfg_hash();
            let cfg = self.configuration_by_hash(cfg_hash).unwrap_or_else(|| {
                panic!("Config with hash {:?} is absent in configs table", cfg_hash)
            });
            Some(cfg)
        } else {
            None
        }
    }

    /// Returns the configuration that is actual for the given height.
    pub fn configuration_by_height(&self, height: Height) -> StoredConfiguration {
        let idx = self.find_configurations_index_by_height(height);
        let cfg_ref = self
            .configs_actual_from()
            .get(idx)
            .unwrap_or_else(|| panic!("Configuration at index {} not found", idx));
        let cfg_hash = cfg_ref.cfg_hash();
        self.configuration_by_hash(cfg_hash)
            .unwrap_or_else(|| panic!("Config with hash {:?} is absent in configs table", cfg_hash))
    }

    /// Returns the configuration for the given configuration hash.
    pub fn configuration_by_hash(&self, hash: &Hash) -> Option<StoredConfiguration> {
        self.configs().get(hash)
    }

    /// Returns the `state_hash` table for core tables.
    pub fn core_state_hash(&self) -> Vec<Hash> {
        vec![
            self.configs().object_hash(),
            self.transaction_results().object_hash(),
        ]
    }

    /// Constructs a proof of inclusion of a root hash of a specific service
    /// table into the block `state_hash`.
    ///
    /// The `service_id` and `table_idx` are automatically combined to form the key of the
    /// required service table; this key serves as a search query for the method.
    /// The service table key is uniquely identified by a `(u16, u16)` tuple
    /// of table coordinates.
    ///
    /// If found, the method returns the root hash as a value of the proof leaf
    /// corresponding to the required service table key. Otherwise, a partial
    /// path to the service table key is returned, which proves its exclusion.
    ///
    /// The resulting proof can be used as a component of proof of state of an
    /// entity stored in the blockchain state at a specific height. The proof is
    /// tied to the `state_hash` of the corresponding `Block`. State of some meta tables
    /// of core and services isn't tracked.
    ///
    /// # Arguments
    ///
    /// * `service_id` - `service_id` as returned by instance of type of
    /// `Service` trait.
    /// * `table_idx` - index of the service table in `Vec`, returned by the
    /// `state_hash` method of an instance of a type of the `Service` trait.
    pub fn get_proof_to_service_table(
        &self,
        service_id: u16,
        table_idx: usize,
    ) -> MapProof<Hash, Hash> {
        let key = Blockchain::service_table_unique_key(service_id, table_idx);
        let sum_table = self.state_hash_aggregator();
        sum_table.get_proof(key)
    }

    /// Saves the given consensus round value into the storage.
    pub(crate) fn set_consensus_round(&mut self, round: Round) {
        let mut entry: Entry<T, _> = Entry::new(CONSENSUS_ROUND, self.access.clone());
        entry.set(round);
    }

    /// Adds a new configuration to the blockchain, which will become actual at
    /// the `actual_from` height in `config_data`.
    pub fn commit_configuration(&mut self, config_data: StoredConfiguration) {
        let actual_from = config_data.actual_from;
        if let Some(last_cfg) = self.configs_actual_from().last() {
            if last_cfg.cfg_hash() != &config_data.previous_cfg_hash {
                // TODO: Replace panic with errors. (ECR-123)
                panic!(
                    "Attempting to commit configuration with incorrect previous hash: {:?}, \
                     expected: {:?}",
                    config_data.previous_cfg_hash,
                    last_cfg.cfg_hash()
                );
            }

            if actual_from <= last_cfg.actual_from() {
                panic!(
                    "Attempting to commit configuration with actual_from {} less than the last \
                     committed the last committed actual_from {}",
                    actual_from,
                    last_cfg.actual_from()
                );
            }
        }

        info!(
            "Scheduled the following configuration for acceptance: {:?}",
            &config_data
        );

        let cfg_hash = config_data.object_hash();
        self.configs().put(&cfg_hash, config_data);

        let cfg_ref = ConfigReference::new(actual_from, &cfg_hash);
        self.configs_actual_from().push(cfg_ref);
    }

    /// Adds transaction into the persistent pool.
    /// This method increment `transactions_pool_len_index`,
    /// be sure to decrement it when transaction committed.
    #[doc(hidden)]
    pub fn add_transaction_into_pool(&mut self, tx: Signed<AnyTx>) {
        self.transactions_pool().insert(tx.object_hash());
        let x = self.transactions_pool_len_index().get().unwrap_or(0);
        self.transactions_pool_len_index().set(x + 1);
        self.transactions().put(&tx.object_hash(), tx);
    }

    /// Changes the transaction status from `in_pool`, to `committed`.
    pub(crate) fn commit_transaction(&mut self, hash: &Hash) {
        self.transactions_pool().remove(hash);
    }

    /// Updates transaction count of the blockchain.
    pub fn update_transaction_count(&mut self, count: u64) {
        let mut len_index = self.transactions_len_index();
        let new_len = len_index.get().unwrap_or(0) + count;
        len_index.set(new_len);
    }

    /// Removes transaction from the persistent pool.
    #[cfg(test)]
    pub(crate) fn reject_transaction(&mut self, hash: &Hash) -> Result<(), ()> {
        let contains = self.transactions_pool().contains(hash);
        self.transactions_pool().remove(hash);
        self.transactions().remove(hash);

        if contains {
            let x = self.transactions_pool_len_index().get().unwrap();
            self.transactions_pool_len_index().set(x - 1);
            Ok(())
        } else {
            Err(())
        }
    }

    fn find_configurations_index_by_height(&self, height: Height) -> u64 {
        let actual_from = self.configs_actual_from();
        for i in (0..actual_from.len()).rev() {
            if actual_from.get(i).unwrap().actual_from() <= height {
                return i as u64;
            }
        }
        panic!(
            "Couldn't not find any config for height {}, \
             that means that genesis block was created incorrectly.",
            height
        )
    }

    /// Returns the next height of the blockchain.
    /// Its value is equal to "height of the latest committed block" + 1.
    fn next_height(&self) -> Height {
        Height(self.block_hashes_by_height().len())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IndexKind {
    /// This index is part of dispatcher schema.
    Dispatcher,
    /// This index is a part of runtime schema.
    Runtime,
    /// This index is a part of some service schema.
    Service(ServiceInstanceId),
}

impl IndexKind {
    /// Returns the corresponding tag.
    fn tag(&self) -> IndexTag {
        match self {
            IndexKind::Dispatcher => IndexTag::Dispatcher,
            IndexKind::Runtime => IndexTag::Runtime,
            IndexKind::Service { .. } => IndexTag::Service,
        }
    }

    /// Returns the corresponding group id.
    fn group_id(&self) -> u32 {
        if let IndexKind::Service(instance_id) = self {
            *instance_id
        } else {
            0
        }
    }
}

/// Binary value for the corresponding index kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u16)]
enum IndexTag {
    Dispatcher = 0,
    Runtime = 1,
    Service = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IndexCoordinates {
    tag: u16,
    group_id: u32,
    index_id: u16,
}

impl IndexCoordinates {
    /// Creates index coordinated for the index with the specified kind and identifier.
    pub fn new(kind: IndexKind, index_id: u16) -> Self {
        Self {
            tag: kind.tag() as u16,
            group_id: kind.group_id(),
            index_id,
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

#[test]
fn test_index_coordinates_binary_key_round_trip() {
    let index_kinds = vec![
        (IndexKind::Dispatcher, 0),
        (IndexKind::Dispatcher, 1),
        (IndexKind::Runtime, 0),
        (IndexKind::Runtime, 5),
        (IndexKind::Service(2), 0),
        (IndexKind::Service(2), 1),
        (IndexKind::Service(0), 0),
        (IndexKind::Service(0), 1),
    ];

    for (kind, id) in index_kinds {
        let coordinate = IndexCoordinates::new(kind, id);
        let mut buf = vec![0; coordinate.size()];
        coordinate.write(&mut buf);

        let coordinate2 = IndexCoordinates::read(&buf);
        assert_eq!(coordinate, coordinate2);        
    }
}