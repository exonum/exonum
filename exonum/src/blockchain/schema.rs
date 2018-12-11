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

use super::{config::StoredConfiguration, Block, BlockProof, Blockchain, TransactionResult};
use crypto::{CryptoHash, Hash, PublicKey};
use helpers::{Height, Round};
use messages::{Connect, Message, Precommit, RawTransaction, Signed};
use proto;
use storage::{
    Entry, Fork, KeySetIndex, ListIndex, MapIndex, MapProof, ProofListIndex, ProofMapIndex,
    Snapshot,
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
    view: T,
}

impl<T> Schema<T>
where
    T: AsRef<dyn Snapshot>,
{
    /// Constructs information schema for the given `snapshot`.
    pub fn new(snapshot: T) -> Self {
        Self { view: snapshot }
    }

    /// Returns a table that represents a map with a key-value pair of a
    /// transaction hash and raw transaction message.
    pub fn transactions(&self) -> MapIndex<&T, Hash, Signed<RawTransaction>> {
        MapIndex::new(TRANSACTIONS, &self.view)
    }

    /// Returns a table that represents a map with a key-value pair of a transaction
    /// hash and execution result.
    ///
    /// This method can be used to retrieve a proof that a certain transaction
    /// result is present in the blockchain.
    pub fn transaction_results(&self) -> ProofMapIndex<&T, Hash, TransactionResult> {
        ProofMapIndex::new(TRANSACTION_RESULTS, &self.view)
    }

    /// Returns a table that represents a set of uncommitted transactions hashes.
    pub fn transactions_pool(&self) -> KeySetIndex<&T, Hash> {
        KeySetIndex::new(TRANSACTIONS_POOL, &self.view)
    }

    /// Returns an entry that represents count of uncommitted transactions.
    pub(crate) fn transactions_pool_len_index(&self) -> Entry<&T, u64> {
        Entry::new(TRANSACTIONS_POOL_LEN, &self.view)
    }

    /// Returns the number of transactions in the pool.
    pub fn transactions_pool_len(&self) -> u64 {
        let pool = self.transactions_pool_len_index();
        pool.get().unwrap_or(0)
    }

    /// Returns a table that keeps the block height and transaction position inside the block for every
    /// transaction hash.
    pub fn transactions_locations(&self) -> MapIndex<&T, Hash, TxLocation> {
        MapIndex::new(TRANSACTIONS_LOCATIONS, &self.view)
    }

    /// Returns a table that stores a block object for every block height.
    pub fn blocks(&self) -> MapIndex<&T, Hash, Block> {
        MapIndex::new(BLOCKS, &self.view)
    }

    /// Returns a table that keeps block hashes for corresponding block heights.
    pub fn block_hashes_by_height(&self) -> ListIndex<&T, Hash> {
        ListIndex::new(BLOCK_HASHES_BY_HEIGHT, &self.view)
    }

    /// Returns a table that keeps a list of transactions for each block.
    pub fn block_transactions(&self, height: Height) -> ProofListIndex<&T, Hash> {
        let height: u64 = height.into();
        ProofListIndex::new_in_family(BLOCK_TRANSACTIONS, &height, &self.view)
    }

    /// Returns a table that keeps a list of precommits for the block with the given hash.
    pub fn precommits(&self, hash: &Hash) -> ListIndex<&T, Signed<Precommit>> {
        ListIndex::new_in_family(PRECOMMITS, hash, &self.view)
    }

    /// Returns a table that represents a map with a key-value pair of a
    /// configuration hash and contents.
    pub fn configs(&self) -> ProofMapIndex<&T, Hash, StoredConfiguration> {
        // configs patricia merkle tree <block height> json
        ProofMapIndex::new(CONFIGS, &self.view)
    }

    /// Returns an auxiliary table that keeps hash references to configurations in
    /// the increasing order of their `actual_from` height.
    pub fn configs_actual_from(&self) -> ListIndex<&T, ConfigReference> {
        ListIndex::new(CONFIGS_ACTUAL_FROM, &self.view)
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
    /// Core tables participate in the resulting state_hash with `CORE_SERVICE`
    /// service_id. Their vector is returned by the `core_state_hash` method.
    pub fn state_hash_aggregator(&self) -> ProofMapIndex<&T, Hash, Hash> {
        ProofMapIndex::new(STATE_HASH_AGGREGATOR, &self.view)
    }

    /// Returns peers that have to be recovered in case of process restart
    /// after abnormal termination.
    pub(crate) fn peers_cache(&self) -> MapIndex<&T, PublicKey, Signed<Connect>> {
        MapIndex::new(PEERS_CACHE, &self.view)
    }

    /// Returns consensus messages that have to be recovered in case of process restart
    /// after abnormal termination.
    pub(crate) fn consensus_messages_cache(&self) -> ListIndex<&T, Message> {
        ListIndex::new(CONSENSUS_MESSAGES_CACHE, &self.view)
    }

    /// Returns the saved value of the consensus round. Returns the first round
    /// if it has not been saved.
    pub(crate) fn consensus_round(&self) -> Round {
        Entry::new(CONSENSUS_ROUND, &self.view)
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
            self.configs().merkle_root(),
            self.transaction_results().merkle_root(),
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

impl<'a> Schema<&'a mut Fork> {
    /// Mutable reference to the [`transactions`][1] index.
    ///
    /// [1]: struct.Schema.html#method.transactions
    pub(crate) fn transactions_mut(&mut self) -> MapIndex<&mut Fork, Hash, Signed<RawTransaction>> {
        MapIndex::new(TRANSACTIONS, self.view)
    }

    /// Mutable reference to the [`transaction_results`][1] index.
    ///
    /// [1]: struct.Schema.html#method.transaction_results
    pub(crate) fn transaction_results_mut(
        &mut self,
    ) -> ProofMapIndex<&mut Fork, Hash, TransactionResult> {
        ProofMapIndex::new(TRANSACTION_RESULTS, self.view)
    }

    /// Mutable reference to the [`transactions_pool`][1] index.
    ///
    /// [1]: struct.Schema.html#method.transactions_pool
    fn transactions_pool_mut(&mut self) -> KeySetIndex<&mut Fork, Hash> {
        KeySetIndex::new(TRANSACTIONS_POOL, self.view)
    }

    /// Mutable reference to the [`transactions_pool_len_index`][1] index.
    ///
    /// [1]: struct.Schema.html#method.transactions_pool_len_index
    pub(crate) fn transactions_pool_len_index_mut(&mut self) -> Entry<&mut Fork, u64> {
        Entry::new(TRANSACTIONS_POOL_LEN, self.view)
    }

    /// Mutable reference to the [`transactions_locations`][1] index.
    ///
    /// [1]: struct.Schema.html#method.transactions_locations
    pub(crate) fn transactions_locations_mut(&mut self) -> MapIndex<&mut Fork, Hash, TxLocation> {
        MapIndex::new(TRANSACTIONS_LOCATIONS, self.view)
    }

    /// Mutable reference to the [`blocks][1] index.
    ///
    /// [1]: struct.Schema.html#method.blocks
    pub(crate) fn blocks_mut(&mut self) -> MapIndex<&mut Fork, Hash, Block> {
        MapIndex::new(BLOCKS, self.view)
    }

    /// Mutable reference to the [`block_hashes_by_height_mut`][1] index.
    ///
    /// [1]: struct.Schema.html#method.block_hashes_by_height_mut
    pub(crate) fn block_hashes_by_height_mut(&mut self) -> ListIndex<&mut Fork, Hash> {
        ListIndex::new(BLOCK_HASHES_BY_HEIGHT, self.view)
    }

    /// Mutable reference to the [`block_transactions`][1] index.
    ///
    /// [1]: struct.Schema.html#method.block_transactions
    pub(crate) fn block_transactions_mut(
        &mut self,
        height: Height,
    ) -> ProofListIndex<&mut Fork, Hash> {
        let height: u64 = height.into();
        ProofListIndex::new_in_family(BLOCK_TRANSACTIONS, &height, self.view)
    }

    /// Mutable reference to the [`precommits`][1] index.
    ///
    /// [1]: struct.Schema.html#method.precommits
    pub(crate) fn precommits_mut(
        &mut self,
        hash: &Hash,
    ) -> ListIndex<&mut Fork, Signed<Precommit>> {
        ListIndex::new_in_family(PRECOMMITS, hash, self.view)
    }

    /// Mutable reference to the [`configs`][1] index.
    ///
    /// [1]: struct.Schema.html#method.configs
    pub(crate) fn configs_mut(&mut self) -> ProofMapIndex<&mut Fork, Hash, StoredConfiguration> {
        ProofMapIndex::new(CONFIGS, self.view)
    }

    /// Mutable reference to the [`configs_actual_from`][1] index.
    ///
    /// [1]: struct.Schema.html#method.configs_actual_from
    pub(crate) fn configs_actual_from_mut(&mut self) -> ListIndex<&mut Fork, ConfigReference> {
        ListIndex::new(CONFIGS_ACTUAL_FROM, self.view)
    }

    /// Mutable reference to the [`state_hash_aggregator`][1] index.
    ///
    /// [1]: struct.Schema.html#method.state_hash_aggregator
    pub(crate) fn state_hash_aggregator_mut(&mut self) -> ProofMapIndex<&mut Fork, Hash, Hash> {
        ProofMapIndex::new(STATE_HASH_AGGREGATOR, self.view)
    }

    /// Mutable reference to the [`peers_cache`][1] index.
    ///
    /// [1]: struct.Schema.html#method.peers_cache
    pub(crate) fn peers_cache_mut(&mut self) -> MapIndex<&mut Fork, PublicKey, Signed<Connect>> {
        MapIndex::new(PEERS_CACHE, self.view)
    }

    /// Mutable reference to the [`consensus_messages_cache`][1] index.
    ///
    /// [1]: struct.Schema.html#method.consensus_messages
    pub(crate) fn consensus_messages_cache_mut(&mut self) -> ListIndex<&mut Fork, Message> {
        ListIndex::new(CONSENSUS_MESSAGES_CACHE, self.view)
    }

    /// Saves the given consensus round value into the storage.
    pub(crate) fn set_consensus_round(&mut self, round: Round) {
        let mut entry: Entry<&mut Fork, _> = Entry::new(CONSENSUS_ROUND, self.view);
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

        let cfg_hash = config_data.hash();
        self.configs_mut().put(&cfg_hash, config_data);

        let cfg_ref = ConfigReference::new(actual_from, &cfg_hash);
        self.configs_actual_from_mut().push(cfg_ref);
    }

    /// Adds transaction into the persistent pool.
    /// This method increment `transactions_pool_len_index`,
    /// be sure to decrement it when transaction committed.
    #[doc(hidden)]
    pub fn add_transaction_into_pool(&mut self, tx: Signed<RawTransaction>) {
        self.transactions_pool_mut().insert(tx.hash());
        let x = self.transactions_pool_len_index().get().unwrap_or(0);
        self.transactions_pool_len_index_mut().set(x + 1);
        self.transactions_mut().put(&tx.hash(), tx);
    }

    /// Changes the transaction status from `in_pool`, to `committed`.
    pub(crate) fn commit_transaction(&mut self, hash: &Hash) {
        self.transactions_pool_mut().remove(hash);
    }

    /// Removes transaction from the persistent pool.
    #[cfg(test)]
    pub(crate) fn reject_transaction(&mut self, hash: &Hash) -> Result<(), ()> {
        let contains = self.transactions_pool_mut().contains(hash);
        self.transactions_pool_mut().remove(hash);
        self.transactions_mut().remove(hash);

        if contains {
            let x = self.transactions_pool_len_index().get().unwrap();
            self.transactions_pool_len_index_mut().set(x - 1);
            Ok(())
        } else {
            Err(())
        }
    }
}
