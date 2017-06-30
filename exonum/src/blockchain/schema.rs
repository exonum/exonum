use byteorder::{ByteOrder, BigEndian};

use std::mem;

use crypto::{Hash};
use messages::{RawMessage, Precommit, BlockProof, CONSENSUS};
use storage::{Snapshot, Fork, StorageKey, StorageValue, ListIndex, MapIndex, ProofListIndex, ProofMapIndex, MapProof};
use super::{Block, Blockchain};
use super::config::StoredConfiguration;

pub fn gen_prefix<K: StorageKey>(service_id: u16, ord: u8, suffix: &K) -> Vec<u8> {
    let pos = mem::size_of::<u16>();
    let mut res = vec![0; pos + 1 + suffix.size()];
    suffix.write(&mut res[pos + 1..]);
    BigEndian::write_u16(&mut res[0..pos], service_id);
    res[pos] = ord;
    res
}

encoding_struct! (
    struct ConfigReference {
        const SIZE = 40;
        field actual_from: u64    [00 => 08]
        field cfg_hash:    &Hash  [08 => 40]
    }
);

encoding_struct! (
    struct TxLocation {
        const SIZE = 16;
        field block_height:         u64  [00 => 08]
        field position_in_block:    u64  [08 => 16]
    }
);

#[derive(Debug)]
pub struct Schema<T> {
    view: T,
}

impl<T> Schema<T> where T: AsRef<Snapshot> {
    pub fn new(view: T) -> Schema<T> {
        Schema { view: view }
    }

    pub fn transactions(&self) -> MapIndex<&T, Hash, RawMessage> {
        MapIndex::new(gen_prefix(CONSENSUS, 0, &()), &self.view)
    }

    pub fn tx_location_by_tx_hash(&self) -> MapIndex<&T, Hash, TxLocation> {
        MapIndex::new(gen_prefix(CONSENSUS, 1, &()), &self.view)
    }

    pub fn blocks(&self) -> MapIndex<&T, Hash, Block> {
        MapIndex::new(gen_prefix(CONSENSUS, 2, &()), &self.view)
    }

    pub fn block_hashes_by_height(&self) -> ListIndex<&T, Hash> {
        ListIndex::new(gen_prefix(CONSENSUS, 3, &()), &self.view)
    }

    pub fn block_hash_by_height(&self, height: u64) -> Option<Hash> {
        self.block_hashes_by_height().get(height)
    }

    pub fn block_txs(&self, height: u64) -> ProofListIndex<&T, Hash> {
        ProofListIndex::new(gen_prefix(CONSENSUS, 4, &height), &self.view)
    }

    pub fn precommits(&self, hash: &Hash) -> ListIndex<&T, Precommit> {
        ListIndex::new(gen_prefix(CONSENSUS, 5, hash), &self.view)
    }

    pub fn block_and_precommits(&self, height: u64) -> Option<BlockProof> {
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

    pub fn configs(&self) -> ProofMapIndex<&T, Hash, StoredConfiguration> {
        // configs patricia merkletree <block height> json
        ProofMapIndex::new(gen_prefix(CONSENSUS, 6, &()), &self.view)
    }

    // TODO: consider List index to reduce storage volume
    pub fn configs_actual_from(&self) -> ListIndex<&T, ConfigReference> {
        ListIndex::new(gen_prefix(CONSENSUS, 7, &()), &self.view)
    }

    pub fn state_hash_aggregator(&self) -> ProofMapIndex<&T, Hash, Hash> {
        ProofMapIndex::new(gen_prefix(CONSENSUS, 8, &()), &self.view)
    }

    pub fn last_block(&self) -> Option<Block> {
        match self.block_hashes_by_height().last() {
           Some(hash) => Some(self.blocks().get(&hash).unwrap()),
           None => None,
        }
    }

    pub fn last_height(&self) -> Option<u64> {
        let block_opt = self.last_block();
        block_opt.map(|block| block.height())
    }

    pub fn current_height(&self) -> u64 {
        let last_height = self.last_height();
        match last_height {
            Some(last_height) => last_height + 1,
            None => 0,
        }
    }

    pub fn actual_configuration(&self) -> StoredConfiguration {
        let current_height = self.current_height();
        let res = self.configuration_by_height(current_height);
        trace!("Retrieved actual_config: {:?}", res);
        res
    }

    pub fn following_configuration(&self) -> Option<StoredConfiguration> {
        let current_height = self.current_height();
        let idx = self.find_configurations_index_by_height(current_height);
        match self.configs_actual_from().get(idx + 1) {
            Some(cfg_ref) => {
                let cfg_hash = cfg_ref.cfg_hash();
                let cfg = self.configuration_by_hash(cfg_hash)
                    .expect(&format!("Config with hash {:?} is absent in configs table", cfg_hash));
                Some(cfg)
            }
            None => None,
        }
    }

    pub fn previous_configuration(&self) -> Option<StoredConfiguration> {
        let current_height = self.current_height();
        let idx = self.find_configurations_index_by_height(current_height);
        if idx > 0 {
            let cfg_ref = self.configs_actual_from()
                .get(idx - 1)
                .expect(&format!("Configuration at index {} not found", idx));
            let cfg_hash = cfg_ref.cfg_hash();
            let cfg =
                self.configuration_by_hash(cfg_hash)
                    .expect(&format!("Config with hash {:?} is absent in configs table", cfg_hash));
            Some(cfg)
        } else {
            None
        }
    }

    pub fn configuration_by_height(&self, height: u64) -> StoredConfiguration {
        let idx = self.find_configurations_index_by_height(height);
        let cfg_ref = self.configs_actual_from()
            .get(idx)
            .expect(&format!("Configuration at index {} not found", idx));
        let cfg_hash = cfg_ref.cfg_hash();
        self.configuration_by_hash(cfg_hash)
                .expect(&format!("Config with hash {:?} is absent in configs table", cfg_hash))
    }

    pub fn configuration_by_hash(&self, hash: &Hash) -> Option<StoredConfiguration> {
        self.configs().get(hash)
    }

    pub fn core_state_hash(&self) -> Vec<Hash> {
        vec![self.configs().root_hash()]
    }

    pub fn get_proof_to_service_table(&self,
                                      service_id: u16,
                                      table_idx: usize)
                                      -> MapProof<Hash> {
        let key = Blockchain::service_table_unique_key(service_id, table_idx);
        let sum_table = self.state_hash_aggregator();
        sum_table.get_proof(&key)
    }

    fn find_configurations_index_by_height(&self, height: u64) -> u64 {
        let actual_from = self.configs_actual_from();
        for i in (0..actual_from.len()).rev() {
            if actual_from.get(i).unwrap().actual_from() <= height {
                return i as u64
            }
        }
        panic!("Couldn't not find any config for height {}, \
                that means that genesis block was created incorrectly.", height)
    }
}

impl<'a> Schema<&'a mut Fork> {
    pub fn transactions_mut(&mut self) -> MapIndex<&mut Fork, Hash, RawMessage> {
        MapIndex::new(gen_prefix(CONSENSUS, 0, &()), &mut self.view)
    }

    pub fn tx_location_by_tx_hash_mut(&mut self) -> MapIndex<&mut Fork, Hash, TxLocation> {
        MapIndex::new(gen_prefix(CONSENSUS, 1, &()), &mut self.view)
    }

    pub fn blocks_mut(&mut self) -> MapIndex<&mut Fork, Hash, Block> {
        MapIndex::new(gen_prefix(CONSENSUS, 2, &()), &mut self.view)
    }

    pub fn block_hashes_by_height_mut(&mut self) -> ListIndex<&mut Fork, Hash> {
        ListIndex::new(gen_prefix(CONSENSUS, 3, &()), &mut self.view)
    }

    pub fn block_hash_by_height_mut(&mut self, height: u64) -> Option<Hash> {
        self.block_hashes_by_height().get(height)
    }

    pub fn block_txs_mut(&mut self, height: u64) -> ProofListIndex<&mut Fork, Hash> {
        ProofListIndex::new(gen_prefix(CONSENSUS, 4, &height), &mut self.view)
    }

    pub fn precommits_mut(&mut self, hash: &Hash) -> ListIndex<&mut Fork, Precommit> {
        ListIndex::new(gen_prefix(CONSENSUS, 5, hash), &mut self.view)
    }

    pub fn configs_mut(&mut self) -> ProofMapIndex<&mut Fork, Hash, StoredConfiguration> {
        // configs patricia merkletree <block height> json
        ProofMapIndex::new(gen_prefix(CONSENSUS, 6, &()), &mut self.view)
    }

    // TODO: consider List index to reduce storage volume
    pub fn configs_actual_from_mut(&mut self) -> ListIndex<&mut Fork, ConfigReference> {
        ListIndex::new(gen_prefix(CONSENSUS, 7, &()), &mut self.view)
    }

    pub fn state_hash_aggregator_mut(&mut self) -> ProofMapIndex<&mut Fork, Hash, Hash> {
        ProofMapIndex::new(gen_prefix(CONSENSUS, 8, &()), &mut self.view)
    }

    pub fn commit_configuration(&mut self, config_data: StoredConfiguration) {
        let actual_from = config_data.actual_from;
        if let Some(last_cfg_reference) = self.configs_actual_from().last() {
            let last_actual_from = last_cfg_reference.actual_from();
            if actual_from <= last_actual_from {
                panic!("Attempting to commit configuration \
                                               with actual_from {:?} less than \
                                              the last committed actual_from {:?}",
                                              actual_from, last_actual_from);
            }
        }
        let cfg_hash = config_data.hash();
        self.configs_mut().put(&cfg_hash, config_data.clone());

        let cfg_ref = ConfigReference::new(actual_from, &cfg_hash);
        self.configs_actual_from_mut().push(cfg_ref);
        info!("Scheduled the following configuration for acceptance: {:?}", config_data);
        // TODO: clear storages
    }

}
