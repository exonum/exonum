use ::crypto::Hash;
use ::messages::{RawMessage, Precommit, BlockProof};
use ::storage::{StorageValue, ListTable, MapTable, MerkleTable, MerklePatriciaTable, HeightBytes,
                Error, Map, List, RootProofNode};

use super::{Block, Blockchain};
use super::config::StoredConfiguration;

pub type ConfigurationData = Vec<u8>;

type StorageView = ::storage::View;

pub struct Schema<'a> {
    view: &'a StorageView,
}

impl<'a> Schema<'a> {
    pub fn new(view: &'a StorageView) -> Schema {
        Schema { view: view }
    }

    pub fn transactions(&self) -> MapTable<StorageView, Hash, RawMessage> {
        MapTable::new(vec![00], self.view)
    }

    pub fn blocks(&self) -> MapTable<StorageView, Hash, Block> {
        MapTable::new(vec![01], self.view)
    }

    pub fn heights(&self) -> ListTable<MapTable<StorageView, [u8], Vec<u8>>, u64, Hash> {
        ListTable::new(MapTable::new(vec![02], self.view))
    }

    pub fn block_txs(&self,
                     height: u64)
                     -> MerkleTable<MapTable<StorageView, [u8], Vec<u8>>, u32, Hash> {
        MerkleTable::new(MapTable::new([&[03u8] as &[u8], &height.serialize()].concat(), self.view))
    }

    pub fn precommits(&self,
                      hash: &Hash)
                      -> ListTable<MapTable<StorageView, [u8], Vec<u8>>, u32, Precommit> {
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
         -> MerklePatriciaTable<MapTable<StorageView, [u8], Vec<u8>>, HeightBytes, ConfigurationData> {
        // configs patricia merkletree <высота блока> json
        MerklePatriciaTable::new(MapTable::new(vec![06], self.view))
    }

    // TODO: consider List index to reduce storage volume
    pub fn configs_heights(&self)
                           -> ListTable<MapTable<StorageView, [u8], Vec<u8>>, u64, HeightBytes> {
        ListTable::new(MapTable::new(vec![07], self.view))
    }

    pub fn state_hash_aggregator(&self)
                            -> MerklePatriciaTable<MapTable<StorageView, [u8], Vec<u8>>, Hash, Hash> {
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

    pub fn get_actual_configuration(&self) -> Result<StoredConfiguration, Error> {
        let h = self.last_height()? + 1;
        let heights = self.configs_heights();
        let height_values = heights.values().unwrap();

        // TODO improve perfomance
        let idx = height_values.into_iter()
            .rposition(|r| u64::from(r) <= h)
            .unwrap();

        let height = heights.get(idx as u64)?.unwrap();
        self.get_configuration_at_height(height.into()).map(|x| x.unwrap())
    }

    // FIXME Replace by result?
    pub fn get_configuration_at_height(&self,
                                       height: u64)
                                       -> Result<Option<StoredConfiguration>, Error> {
        let configs = self.configs();
        if let Some(config) = configs.get(&height.into())? {
            match StoredConfiguration::deserialize(&config) {
                Ok(configuration) => {
                    return Ok(Some(configuration));
                }
                Err(_) => {
                    error!("Can't parse found configuration at height: {}", height);
                }
            }
        }
        Ok(None)
    }

    pub fn core_state_hash(&self) -> Result<Vec<Hash>, Error> {
        Ok(vec![self.configs().root_hash()?])
    }

    pub fn get_proof_to_service_table(&self, service_id: u16, table_idx: usize) -> Result<RootProofNode<Hash>, Error>
    {
        let key = Blockchain::service_table_unique_key(service_id, table_idx); 
        let sum_table = self.state_hash_aggregator(); 
        sum_table.construct_path_to_key(key)
    }
}