use crypto::{Hash, hash};
use messages::{RawMessage, Precommit, BlockProof};
use storage::{StorageValue, ListTable, MapTable, MerkleTable, MerklePatriciaTable, Error, Map,
              List, RootProofNode, View};

use super::{Block, Blockchain};
use super::config::StoredConfiguration;

storage_value! (
    ConfigReference {
        const SIZE = 40;
        actual_from: u64    [00 => 08]
        cfg_hash:    &Hash  [08 => 40]
    }
);


pub struct Schema<'a> {
    view: &'a View,
}

impl<'a> Schema<'a> {
    pub fn new(view: &'a View) -> Schema {
        Schema { view: view }
    }

    pub fn transactions(&self) -> MapTable<View, Hash, RawMessage> {
        MapTable::new(vec![00], self.view)
    }

    pub fn blocks(&self) -> MapTable<View, Hash, Block> {
        MapTable::new(vec![01], self.view)
    }

    pub fn heights(&self) -> ListTable<MapTable<View, [u8], Vec<u8>>, u64, Hash> {
        ListTable::new(MapTable::new(vec![02], self.view))
    }

    pub fn block_txs(&self, height: u64) -> MerkleTable<MapTable<View, [u8], Vec<u8>>, u32, Hash> {
        MerkleTable::new(MapTable::new([&[03u8] as &[u8], &height.serialize()].concat(), self.view))
    }

    pub fn precommits(&self,
                      hash: &Hash)
                      -> ListTable<MapTable<View, [u8], Vec<u8>>, u32, Precommit> {
        ListTable::new(MapTable::new([&[03], hash.as_ref()].concat(), self.view))
    }

    pub fn block_and_precommits(&self, height: u64) -> Result<Option<BlockProof>, Error> {
        let block_hash = match self.heights().get(height)? {
            None => return Ok(None),
            Some(block_hash) => block_hash,
        };
        let block = self.blocks().get(&block_hash)?.unwrap();
        let precommits_table = self.precommits(&block_hash);
        let precommits = precommits_table.values()?;
        let res = BlockProof {
            block: block,
            precommits: precommits,
        };
        Ok(Some(res))
    }

    pub fn configs
        (&self)
         -> MerklePatriciaTable<MapTable<View, [u8], Vec<u8>>, Hash, StoredConfiguration> {
        // configs patricia merkletree <высота блока> json
        MerklePatriciaTable::new(MapTable::new(vec![06], self.view))
    }

    // TODO: consider List index to reduce storage volume
    pub fn configs_actual_from
        (&self)
         -> ListTable<MapTable<View, [u8], Vec<u8>>, u64, ConfigReference> {
        ListTable::new(MapTable::new(vec![07], self.view))
    }

    pub fn state_hash_aggregator
        (&self)
         -> MerklePatriciaTable<MapTable<View, [u8], Vec<u8>>, Hash, Hash> {
        MerklePatriciaTable::new(MapTable::new(vec![08], self.view))
    }

    pub fn last_block(&self) -> Result<Option<Block>, Error> {
        Ok(match self.heights().last()? {
            Some(hash) => Some(self.blocks().get(&hash)?.unwrap()),
            None => None,
        })
    }

    pub fn last_height(&self) -> Result<u64, Error> {
        self.last_block().map(|block| block.unwrap().height())
    }

    pub fn commit_actual_configuration(&self,
                                       config_data: StoredConfiguration)
                                       -> Result<(), Error> {
        let actual_from = config_data.actual_from;
        if actual_from > 0 {
            let last_actual_from = self.configs_actual_from()
                .last()?
                .expect("configs_actual_from table returned None on last()")
                .actual_from();
            if actual_from <= last_actual_from {
                return Err(Error::new(format!("Attempting to commit configuration with actual_from {:?} less than \
                                              the last committed actual_from {:?}",  actual_from, last_actual_from)));
            }
        }
        let cfg_hash = config_data.hash();

        self.configs().put(&cfg_hash, config_data)?;
        let cfg_ref = ConfigReference::new(actual_from, &cfg_hash);
        self.configs_actual_from().append(cfg_ref)?;
        // TODO: clear storages
        Ok(())
    }

    pub fn get_actual_configurations_index(&self) -> Result<u64, Error> {
        let h = self.last_height()? + 1;
        let configs_actual_from = self.configs_actual_from();
        let cfg_references: Vec<ConfigReference> = configs_actual_from.values()?;

        let idx = cfg_references.into_iter()
         .rposition(|r| r.actual_from() <= h)
         .expect(&format!("Couldn't find a config in configs_actual_from table with actual_from height less than \
                          the current height: {:?}", h));
        Ok(idx as u64)
    }

    pub fn get_actual_configuration(&self) -> Result<StoredConfiguration, Error> {
        let idx = self.get_actual_configurations_index()?;
        let cfg_ref: ConfigReference = self.configs_actual_from()
            .get(idx)?
            .expect(&format!("No element at idx {:?} in configs_actual_from table", idx));
        let cfg_hash = cfg_ref.cfg_hash();
        let res = self.get_configuration_by_hash(cfg_hash).map(|x| {
            x.expect(&format!("Config with hash {:?} is absent in configs table", cfg_hash))
        });
        trace!("Retrieved actual_config: {:?}", res);
        res
    }

    pub fn get_following_configuration(&self) -> Result<Option<StoredConfiguration>, Error> {
        let idx = self.get_actual_configurations_index()?;
        let res = match self.configs_actual_from().get(idx + 1)? {
            Some(cfg_ref) => {
                let cfg_hash = cfg_ref.cfg_hash();
                let cfg = self.get_configuration_by_hash(cfg_hash)?
                    .expect(&format!("Config with hash {:?} is absent in configs table", cfg_hash));
                Some(cfg)
            }
            None => None,
        };
        Ok(res)
    }

    pub fn get_configuration_by_hash(&self,
                                     hash: &Hash)
                                     -> Result<Option<StoredConfiguration>, Error> {
        self.configs().get(hash)
    }

    pub fn core_state_hash(&self) -> Result<Vec<Hash>, Error> {
        Ok(vec![self.configs().root_hash()?])
    }

    pub fn get_proof_to_service_table(&self,
                                      service_id: u16,
                                      table_idx: usize)
                                      -> Result<RootProofNode<Hash>, Error> {
        let key = Blockchain::service_table_unique_key(service_id, table_idx);
        let sum_table = self.state_hash_aggregator();
        sum_table.construct_path_to_key(key)
    }
}
