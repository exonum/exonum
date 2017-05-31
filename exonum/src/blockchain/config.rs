//! Exonum global variables.
//! TODO

use serde_json;

use std::collections::BTreeMap;

use storage::StorageValue;
use events::Milliseconds;
use crypto::{hash, PublicKey, Hash};

/// Exonum blockchain global configuration.
/// This configuration must be same for any exonum node in the certain network on given height.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StoredConfiguration {
    /// Link to the previous configuration. 
    /// For configuration in genesis block `hash` is just an array of zeroes.
    pub previous_cfg_hash: Hash, 
    /// The height with which this configuration becomes an actual.
    pub actual_from: u64,
    /// List of validator's public keys
    pub validators: Vec<PublicKey>,
    /// Consensus algorithm parameters.
    pub consensus: ConsensusConfig,
    /// Services specific variables.
    /// Keys are `service_id` from `Service` trait and values are the serialized json.
    pub services: BTreeMap<String, serde_json::Value>,
}

/// Consensus algorithm parameters.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ConsensusConfig {
    /// Interval (ms) between rounds.
    pub round_timeout: Milliseconds,
    /// Period (ms) of sending a Status message.
    pub status_timeout: Milliseconds,
    /// Peer exchange timeout (in ms).
    pub peers_timeout: Milliseconds,
    /// Proposal timeout (ms) after the new height beginning.
    pub propose_timeout: Milliseconds,
    /// Maximum number of transactions per block.
    pub txs_block_limit: u32,
}

impl Default for ConsensusConfig {
    fn default() -> Self {
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
    /// Tries to serialize given configuration into the utf8 encoded string.
    pub fn try_serialize(&self) -> Result<Vec<u8>, serde_json::error::Error> {
        serde_json::to_vec(&self)
    }

    /// Tries to deserialize `StorageConfiguration` from the given utf8 encoded byte array.
    pub fn try_deserialize(serialized: &[u8]) -> Result<StoredConfiguration, serde_json::error::Error> {
        serde_json::from_slice(serialized)
    }
}

impl StorageValue for StoredConfiguration {
    fn serialize(self) -> Vec<u8> {
        self.try_serialize().unwrap()
    }

    fn deserialize(v: Vec<u8>) -> Self {
        StoredConfiguration::try_deserialize(&v).unwrap()
    }

    fn hash(&self) -> Hash {
        let vec_bytes = self.try_serialize().unwrap();
        hash(&vec_bytes)
    }

}
