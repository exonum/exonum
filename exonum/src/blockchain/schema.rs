// Copyright 2017 The Exonum Team
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

use crypto::Hash;
use messages::{RawMessage, Precommit};
use storage::{Snapshot, Fork, StorageKey, StorageValue, ListIndex, MapIndex, ProofListIndex,
              ProofMapIndex, MapProof};
use helpers::Height;
use super::{Block, BlockProof, Blockchain};
use super::config::StoredConfiguration;

/// Generates an array of bytes from the `prefix`.
pub fn gen_prefix<K: StorageKey>(prefix: &K) -> Vec<u8> {
    let mut res = vec![0; prefix.size()];
    prefix.write(&mut res[..]);
    res
}

encoding_struct! (
    /// Configuration index.
    struct ConfigReference {
        const SIZE = 40;
        /// The height, starting from which this configuration becomes actual.
        field actual_from: Height [00 => 08]
        /// Hash of the configuration contents that serialized as raw bytes vec.
        field cfg_hash:    &Hash  [08 => 40]
    }
);

encoding_struct! (
    /// Transaction location in block.
    struct TxLocation {
        const SIZE = 16;
        /// Height of block in the blockchain.
        field block_height:         Height  [00 => 08]
        /// Index in block.
        field position_in_block:    u64     [08 => 16]
    }
);

/// Information schema for `exonum-core`.
#[derive(Debug)]
pub struct Schema<T> {
    view: T,
}

impl<T> Schema<T>
where
    T: AsRef<Snapshot>,
{
    /// Constructs information schema for the given `snapshot`.
    pub fn new(snapshot: T) -> Schema<T> {
        Schema { view: snapshot }
    }

    /// Returns table that represents a map from transaction hash into raw transaction message.
    pub fn transactions(&self) -> MapIndex<&T, Hash, RawMessage> {
        MapIndex::new("core.transactions", &self.view)
    }

    /// Returns table that keeps the block height and tx position inside block for every
    /// transaction hash.
    pub fn tx_location_by_tx_hash(&self) -> MapIndex<&T, Hash, TxLocation> {
        MapIndex::new("core.tx_location_by_tx_hash", &self.view)
    }

    /// Returns table that stores block object for every block height.
    pub fn blocks(&self) -> MapIndex<&T, Hash, Block> {
        MapIndex::new("core.blocks", &self.view)
    }

    /// Returns table that keeps block hash for the corresponding height.
    pub fn block_hashes_by_height(&self) -> ListIndex<&T, Hash> {
        ListIndex::new("core.block_hashes_by_height", &self.view)
    }

    /// Returns table that keeps a list of transactions for the each block.
    pub fn block_txs(&self, height: Height) -> ProofListIndex<&T, Hash> {
        let height: u64 = height.into();
        ProofListIndex::with_prefix("core.block_txs", gen_prefix(&height), &self.view)
    }

    /// Returns table that saves a list of precommits for block with given hash.
    pub fn precommits(&self, hash: &Hash) -> ListIndex<&T, Precommit> {
        ListIndex::with_prefix("core.precommits", gen_prefix(hash), &self.view)
    }

    /// Returns table that represents a map from configuration hash into contents.
    pub fn configs(&self) -> ProofMapIndex<&T, Hash, StoredConfiguration> {
        // configs patricia merkletree <block height> json
        ProofMapIndex::new("core.configs", &self.view)
    }

    /// Returns auxiliary table that keeps hash references to configurations in order
    /// of increasing their `actual_from` height.
    pub fn configs_actual_from(&self) -> ListIndex<&T, ConfigReference> {
        ListIndex::new("core.configs_actual_from", &self.view)
    }

    /// Returns the accessory `ProofMapIndex` for calculating
    /// patches in the DBView layer.
    ///
    /// Table calculates "aggregation" of root hashes of individual
    /// service tables, in effect summing the state of various entities,
    /// scattered across distinct services and their tables. Sum is performed by
    /// means of computing root hash of this table.
    ///
    /// - Table **key** is 32 bytes of normalized coordinates of a service
    /// table, as returned by `service_table_unique_key` helper function.
    /// - Table **value** is root hash of a service table, which contributes
    /// to the resulting block's `state_hash`.
    ///
    /// Core tables participate in resulting state_hash with `CORE_SERVICE`
    /// service_id. Their vector is returned by `core_state_hash` method.
    pub fn state_hash_aggregator(&self) -> ProofMapIndex<&T, Hash, Hash> {
        ProofMapIndex::new("core.state_hash_aggregator", &self.view)
    }

    /// Returns block hash for the given height.
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
        let res = BlockProof {
            block: block,
            precommits: precommits,
        };
        Some(res)
    }

    /// Returns latest committed block.
    pub fn last_block(&self) -> Option<Block> {
        match self.block_hashes_by_height().last() {
            Some(hash) => Some(self.blocks().get(&hash).unwrap()),
            None => None,
        }
    }

    /// Returns height of the latest committed block.
    pub fn last_height(&self) -> Option<Height> {
        let block_opt = self.last_block();
        block_opt.map(|block| block.height())
    }

    /// Returns the current height of the blockchain. Its value is equal to `last_height + 1`.
    pub fn current_height(&self) -> Height {
        let last_height = self.last_height();
        match last_height {
            Some(last_height) => last_height.next(),
            None => Height::zero(),
        }
    }

    /// Returns configuration for the latest height of blockchain.
    pub fn actual_configuration(&self) -> StoredConfiguration {
        let current_height = self.current_height();
        let res = self.configuration_by_height(current_height);
        trace!("Retrieved actual_config: {:?}", res);
        res
    }

    /// Returns the nearest following configuration if it exists.
    pub fn following_configuration(&self) -> Option<StoredConfiguration> {
        let current_height = self.current_height();
        let idx = self.find_configurations_index_by_height(current_height);
        match self.configs_actual_from().get(idx + 1) {
            Some(cfg_ref) => {
                let cfg_hash = cfg_ref.cfg_hash();
                let cfg = self.configuration_by_hash(cfg_hash).expect(&format!(
                    "Config with hash {:?} is absent in configs table",
                    cfg_hash
                ));
                Some(cfg)
            }
            None => None,
        }
    }

    /// Returns the previous configuration if it exists.
    pub fn previous_configuration(&self) -> Option<StoredConfiguration> {
        let current_height = self.current_height();
        let idx = self.find_configurations_index_by_height(current_height);
        if idx > 0 {
            let cfg_ref = self.configs_actual_from().get(idx - 1).expect(&format!(
                "Configuration at index {} not found",
                idx
            ));
            let cfg_hash = cfg_ref.cfg_hash();
            let cfg = self.configuration_by_hash(cfg_hash).expect(&format!(
                "Config with hash {:?} is absent in configs table",
                cfg_hash
            ));
            Some(cfg)
        } else {
            None
        }
    }

    /// Returns the configuration that is the actual for the given height.
    pub fn configuration_by_height(&self, height: Height) -> StoredConfiguration {
        let idx = self.find_configurations_index_by_height(height);
        let cfg_ref = self.configs_actual_from().get(idx).expect(&format!(
            "Configuration at index {} not found",
            idx
        ));
        let cfg_hash = cfg_ref.cfg_hash();
        self.configuration_by_hash(cfg_hash).expect(&format!(
            "Config with hash {:?} is absent in configs table",
            cfg_hash
        ))
    }

    /// Returns configuration for given configuration hash.
    pub fn configuration_by_hash(&self, hash: &Hash) -> Option<StoredConfiguration> {
        self.configs().get(hash)
    }

    /// Returns the `state_hash` table for core tables.
    pub fn core_state_hash(&self) -> Vec<Hash> {
        vec![self.configs().root_hash()]
    }

    /// Constructs a proof of inclusion of root hash of a specific service
    /// table into block's `state_hash`.
    ///
    /// Searched key for proof is uniquely identified by (`u16`, `u16`) tuple
    /// of table's coordinates.
    ///
    /// If found, root hash is returned as a value of proof's leaf
    /// corresponding to searched key. Otherwise, partial path to searched key
    /// is returned, which proves its exclusion.
    ///
    /// The returned proof is used as a component of proof of state of any
    /// entity, stored in `exonum` db at specific height, as identified
    /// by corresponding block's `state_hash`. State of some meta tables
    /// of core and services isn't tracked.
    ///
    /// # Arguments
    ///
    /// * `service_id` - `service_id` as returned by instance of type of
    /// `Service` trait
    /// * `table_idx` - index of service table in `Vec`, returned by
    /// `state_hash` method of instance of type of `Service` trait
    pub fn get_proof_to_service_table(&self, service_id: u16, table_idx: usize) -> MapProof<Hash> {
        let key = Blockchain::service_table_unique_key(service_id, table_idx);
        let sum_table = self.state_hash_aggregator();
        sum_table.get_proof(&key)
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
}

impl<'a> Schema<&'a mut Fork> {
    /// Mutable reference to the [`transactions`][1] index.
    ///
    /// [1]: struct.Schema.html#method.transactions
    pub fn transactions_mut(&mut self) -> MapIndex<&mut Fork, Hash, RawMessage> {
        MapIndex::new("core.transactions", &mut self.view)
    }

    /// Mutable reference to the [`tx_location_by_tx_hash`][1] index.
    ///
    /// [1]: struct.Schema.html#method.tx_location_by_tx_hash
    pub fn tx_location_by_tx_hash_mut(&mut self) -> MapIndex<&mut Fork, Hash, TxLocation> {
        MapIndex::new("core.tx_location_by_tx_hash", &mut self.view)
    }

    /// Mutable reference to the [`blocks][1] index.
    ///
    /// [1]: struct.Schema.html#method.blocks
    pub fn blocks_mut(&mut self) -> MapIndex<&mut Fork, Hash, Block> {
        MapIndex::new("core.blocks", &mut self.view)
    }

    /// Mutable reference to the [`block_hashes_by_height_mut`][1] index.
    ///
    /// [1]: struct.Schema.html#method.block_hashes_by_height_mut
    pub fn block_hashes_by_height_mut(&mut self) -> ListIndex<&mut Fork, Hash> {
        ListIndex::new("core.block_hashes_by_height", &mut self.view)
    }

    /// Mutable reference to the [`block_hash_by_height`][1] index.
    ///
    /// [1]: struct.Schema.html#method.block_hash_by_height
    pub fn block_hash_by_height_mut(&mut self, height: Height) -> Option<Hash> {
        self.block_hashes_by_height().get(height.into())
    }

    /// Mutable reference to the [`block_txs`][1] index.
    ///
    /// [1]: struct.Schema.html#method.block_txs
    pub fn block_txs_mut(&mut self, height: Height) -> ProofListIndex<&mut Fork, Hash> {
        let height: u64 = height.into();
        ProofListIndex::with_prefix("core.block_txs", gen_prefix(&height), &mut self.view)
    }

    /// Mutable reference to the [`precommits`][1] index.
    ///
    /// [1]: struct.Schema.html#method.precommits
    pub fn precommits_mut(&mut self, hash: &Hash) -> ListIndex<&mut Fork, Precommit> {
        ListIndex::with_prefix("core.precommits", gen_prefix(hash), &mut self.view)
    }

    /// Mutable reference to the [`configs`][1] index.
    ///
    /// [1]: struct.Schema.html#method.configs
    pub fn configs_mut(&mut self) -> ProofMapIndex<&mut Fork, Hash, StoredConfiguration> {
        ProofMapIndex::new("core.configs", &mut self.view)
    }

    /// Mutable reference to the [`configs_actual_from`][1] index.
    ///
    /// [1]: struct.Schema.html#method.configs_actual_from
    pub fn configs_actual_from_mut(&mut self) -> ListIndex<&mut Fork, ConfigReference> {
        ListIndex::new("core.configs_actual_from", &mut self.view)
    }

    /// Mutable reference to the [`state_hash_aggregator`][1] index.
    ///
    /// [1]: struct.Schema.html#method.state_hash_aggregator
    pub fn state_hash_aggregator_mut(&mut self) -> ProofMapIndex<&mut Fork, Hash, Hash> {
        ProofMapIndex::new("core.state_hash_aggregator", &mut self.view)
    }

    /// Adds a new configuration to the blockchain, which will become an actual at
    /// the `actual_from` height in `config_data`.
    pub fn commit_configuration(&mut self, config_data: StoredConfiguration) {
        let actual_from = config_data.actual_from;
        if let Some(last_cfg) = self.configs_actual_from().last() {
            if last_cfg.cfg_hash() != &config_data.previous_cfg_hash {
                // TODO: Replace panic with errors (ECR-123).
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

        let cfg_hash = config_data.hash();
        self.configs_mut().put(&cfg_hash, config_data.clone());

        let cfg_ref = ConfigReference::new(actual_from, &cfg_hash);
        self.configs_actual_from_mut().push(cfg_ref);
        info!(
            "Scheduled the following configuration for acceptance: {:?}",
            config_data
        );
        // TODO: clear storages
    }
}
