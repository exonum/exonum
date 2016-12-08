use std::ops::Deref;
use config::serde_json;

use super::super::crypto::{Hash, PublicKey};
use ::blockchain::View;
use ::storage::{Fork, MapTable, MerklePatriciaTable};
use config::txs::{ConfigTx, TxConfigPropose, TxConfigVote};
use config::HeightBytecode;
use config::ConfigurationData;

pub struct ConfigsView<F: Fork> {
    pub fork: F,
}

impl<F> View<F> for ConfigsView<F> where F: Fork
{
    type Transaction = ConfigTx;

    fn from_fork(fork: F) -> Self {
        ConfigsView { fork: fork }
    }
}

impl<F> Deref for ConfigsView<F> where F: Fork
{
    type Target = F;

    fn deref(&self) -> &Self::Target {
        &self.fork
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StoredConfiguration {
    actual_from: u64,
    pub validators: Vec<PublicKey>,
    pub consensus: ConsensusCfg
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ConsensusCfg {
    pub round_timeout: i64,    // 2000
    pub status_timeout: i64,   // 5000
    pub peers_timeout: i64,    // 10000
    pub propose_timeout: i64,  // 500
    pub txs_block_limit: u32   // 500
}

impl StoredConfiguration {

    #[allow(dead_code)]
    pub fn serialize(&self) -> Vec<u8> {
        serde_json::to_vec(&self).unwrap()
    }

    #[allow(dead_code)]
    pub fn deserialize(serialized: &[u8]) -> Result<StoredConfiguration, &str> {
        let cfg: StoredConfiguration = serde_json::from_slice(serialized).unwrap();
        if cfg.is_valid() {
            return Ok(cfg);
        }
        Err("not valid")
    }    

    fn is_valid(&self) -> bool {
        // TODO: some validations if it's needed
        true
    }
}

impl<F> ConfigsView<F> where F: Fork
{
    pub fn config_proposes(&self) -> MerklePatriciaTable<MapTable<F, [u8], Vec<u8>>, Hash, TxConfigPropose> {
        //config_propose paricia merkletree <hash_tx> транзакция пропоз
        MerklePatriciaTable::new(MapTable::new(vec![04], self))
    }

    pub fn config_votes(&self) -> MerklePatriciaTable<MapTable<F, [u8], Vec<u8>>, PublicKey, TxConfigVote> {
        //config_votes patricia merkletree <pub_key> последний голос
        MerklePatriciaTable::new(MapTable::new(vec![05], self))
    }

    pub fn configs(&self) -> MerklePatriciaTable<MapTable<F, [u8], Vec<u8>>, HeightBytecode, ConfigurationData> {
        //configs patricia merkletree <высота блока> json
        MerklePatriciaTable::new(MapTable::new(vec![06], self))
    }
}