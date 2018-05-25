use std::collections::HashMap;
use std::net::SocketAddr;
use crypto::PublicKey;
use blockchain::ValidatorKeys;

/// doc will be here
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConnectList {
    /// doc will be here
    pub peers: HashMap<SocketAddr, PublicKey>,
}

impl ConnectList {

    /// Create ConnectList from validators_keys and peers
    pub fn from(validators_keys:&Vec<ValidatorKeys>, peers: &Vec<SocketAddr>) -> Self {
        let peers: HashMap<SocketAddr, PublicKey> = peers.iter().zip(validators_keys.iter()).map(|(p, v)| {
            (*p, v.consensus_key)
        }).collect();

        ConnectList {
            peers
        }
    }

}
