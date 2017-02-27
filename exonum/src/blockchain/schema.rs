use crypto::Hash;
use messages::{RawMessage, Precommit, BlockProof};
use storage::{StorageValue, ListTable, MapTable, MerkleTable, MerklePatriciaTable, HeightBytes,
              Error, Map, List, RootProofNode, View};

use super::{Block, Blockchain};
use super::config::StoredConfiguration;

pub type ConfigurationData = Vec<u8>;

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

    pub fn block_txs(&self,
                     height: u64)
                     -> MerkleTable<MapTable<View, [u8], Vec<u8>>, u32, Hash> {
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
         -> MerklePatriciaTable<MapTable<View, [u8], Vec<u8>>, HeightBytes, ConfigurationData> {
        // configs patricia merkletree <высота блока> json
        MerklePatriciaTable::new(MapTable::new(vec![06], self.view))
    }

    // TODO: consider List index to reduce storage volume
    pub fn configs_heights(&self)
                           -> ListTable<MapTable<View, [u8], Vec<u8>>, u64, HeightBytes> {
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
                                       actual_from: u64,
                                       config_data: &[u8])
                                       -> Result<(), Error> {
        let height_bytecode = actual_from.into();
        self.configs().put(&height_bytecode, config_data.to_vec())?;
        self.configs_heights().append(height_bytecode)?;
        // TODO: clear storages
        Ok(())
    }

    pub fn get_actual_configurations_index(&self) -> Result<u64, Error> {
        // TODO improve perfomance, use iterators and binary search
        let h = self.last_height()? + 1;
        let heights = self.configs_heights();
        let height_values = heights.values().unwrap();

        let idx = height_values.into_iter()
            .rposition(|r| u64::from(r) <= h)
            .unwrap();
        Ok(idx as u64)
    }

    pub fn get_actual_configuration(&self) -> Result<StoredConfiguration, Error> {
        let idx = self.get_actual_configurations_index()?;
        let height = self.configs_heights()
            .get(idx)?
            .unwrap();
        self.get_configuration_at_height(height).map(|x| x.unwrap())
    }

    pub fn get_following_configuration(&self) -> Result<Option<StoredConfiguration>, Error> {
        let idx = self.get_actual_configurations_index()?;
        if let Some(height) = self.configs_heights().get(idx + 1)? {
            self.get_configuration_at_height(height)
        } else {
            Ok(None)
        }
    }

    pub fn get_configuration_at_height<H>(&self,
                                          height: H)
                                          -> Result<Option<StoredConfiguration>, Error>
        where H: Into<HeightBytes>
    {
        let configs = self.configs();
        let height = height.into();
        if let Some(config) = configs.get(&height)? {
            match StoredConfiguration::deserialize(&config) {
                Ok(configuration) => {
                    return Ok(Some(configuration));
                }
                Err(_) => {
                    error!("Can't parse found configuration at height: {}", u64::from(height));
                }
            }
        }
        Ok(None)
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