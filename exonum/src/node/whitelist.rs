//\TODO don't reload whitelisted_peers if path the same 
use std::collections::BTreeSet;

use crypto::PublicKey;

/// `Whitelist` is special set to keep peers that can connect to us.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Whitelist {
    whitelist_on: bool,
    whitelisted_peers: BTreeSet<PublicKey>,

    #[serde(default)]
    #[serde(skip_serializing)]
    #[serde(skip_deserializing)]
    // keep validators independent, to allow easy update
    validators_list: BTreeSet<PublicKey>,
}



impl Whitelist {
    /// is this `peer` can connect or not
    pub fn contains(& self, peer: &PublicKey) -> bool {
        !self.whitelist_on || self.validators_list.contains(peer)
        || self.whitelisted_peers.contains(peer)
    }

    /// append `peer` to whitelist
    pub fn set(&mut self, peer: PublicKey) {
        self.whitelisted_peers.insert(peer);
    }

    /// get list of peers in whitelist
    pub fn get_whitelist(&self) -> Vec<&PublicKey> {
        self.whitelisted_peers.iter()
                              .chain(self.validators_list.iter())
                              .collect()
    }

    pub fn update_validators<I>(&mut self, list: I)
        where I:IntoIterator<Item=PublicKey>
    {
        self.validators_list = list.into_iter().collect();
    }

    /// check if we support whitelist, or keep connection politics open
    /// if it return true, everybody can connect to us
    pub fn is_whitelist_disabled(&self) -> bool {
        !self.whitelist_on
    }
}