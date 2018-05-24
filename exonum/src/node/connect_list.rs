use std::collections::HashMap;
use std::net::SocketAddr;
use crypto::PublicKey;

/// doc will be here
#[derive(Debug, Default, Clone)]
pub struct ConnectList {
    /// doc will be here
    pub peers: HashMap<SocketAddr, PublicKey>,
}
