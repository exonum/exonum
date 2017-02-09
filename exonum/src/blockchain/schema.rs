use ::crypto::Hash;
use ::messages::{RawMessage, Precommit};
use ::storage::{StorageValue, ListTable, MapTable, MerkleTable, MerklePatriciaTable,
                Error, Map, List};

use super::Block;
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

    pub fn transactions(&self) -> MapTable<Hash, RawMessage> {
        MapTable::new(vec![00], self.view)
    }

    pub fn blocks(&self) -> MapTable<Hash, Block> {
        MapTable::new(vec![01], self.view)
    }

    pub fn heights(&self) -> ListTable<'a, Hash> {
        ListTable::new(vec![02], self.view)
    }

    pub fn block_txs(&self, height: u64) -> MerkleTable<'a, Hash> {
        MerkleTable::new([&[03u8] as &[u8], &height.serialize()].concat(), self.view)
    }

    pub fn precommits(&self, hash: &Hash) -> ListTable<'a, Precommit> {
        ListTable::new([&[03], hash.as_ref()].concat(), self.view)
    }

    pub fn configs
        (&self)
         -> MerklePatriciaTable<'a, u64, ConfigurationData> {
        // configs patricia merkletree <высота блока> json
        MerklePatriciaTable::new(vec![06], self.view)
    }

    // TODO: consider List index to reduce storage volume
    pub fn configs_heights(&self) -> ListTable<'a, u64> {
        ListTable::new(vec![07], self.view)
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

    pub fn state_hash(&self) -> Result<Hash, Error> {
        self.configs().root_hash()      
    }
}