use serde_json;

use std::collections::HashMap;

use serde_json::Value;

use ::crypto::PublicKey;

#[derive(Debug, Serialize, Deserialize)]
pub struct StoredConfiguration {
    pub actual_from: u64,
    pub validators: Vec<PublicKey>,
    pub consensus: ConsensusConfig,
    pub services: HashMap<u16, Value>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ConsensusConfig {
    pub round_timeout: i64,
    pub status_timeout: i64,
    pub peers_timeout: i64,
    pub propose_timeout: i64,
    pub txs_block_limit: u32,
}

impl Default for ConsensusConfig {
    fn default() -> ConsensusConfig {
        ConsensusConfig {
            round_timeout: 3000,
            propose_timeout: 500,
            status_timeout: 5000,
            peers_timeout: 10000,
            txs_block_limit: 1000,
        }
    }
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
        self.consensus.round_timeout < 10000
    }
}
