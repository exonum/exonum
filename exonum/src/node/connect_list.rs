use blockchain::ValidatorKeys;
use crypto::PublicKey;
use helpers::fabric::NodePublicConfig;
use std::collections::HashMap;
use std::net::SocketAddr;

/// doc will be here
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConnectList {
    /// doc will be here
    pub peers: HashMap<SocketAddr, PublicKey>,
}

impl ConnectList {
    /// Create ConnectList from validators_keys and peers
    pub fn from_keys(validators_keys: &Vec<ValidatorKeys>, peers: &Vec<SocketAddr>) -> Self {
        let peers: HashMap<SocketAddr, PublicKey> = peers
            .iter()
            .zip(validators_keys.iter())
            .map(|(p, v)| (*p, v.consensus_key))
            .collect();

        ConnectList { peers }
    }

    /// Create ConnectList from NodePublicConfigs
    pub fn from_node_config(list: &Vec<NodePublicConfig>) -> Self {
        let peers: HashMap<SocketAddr, PublicKey> = list.iter()
            .map(|config| (config.addr, config.validator_keys.consensus_key))
            .collect();
        ConnectList { peers }
    }
}
