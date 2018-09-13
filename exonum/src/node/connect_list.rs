// Copyright 2018 The Exonum Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Mapping between peers public keys and IP-addresses.

use std::{collections::BTreeMap, net::SocketAddr, net::ToSocketAddrs};

use crypto::PublicKey;
use node::{ConnectInfo, ConnectListConfig};

/// Network address of the peer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerAddress {
    /// External address of the peer hostname:port.
    pub address: String,
    resolved: Option<SocketAddr>,
}

impl PeerAddress {
    /// New unresolved address.
    pub fn new(address: String) -> Self {
        PeerAddress {
            address,
            resolved: None,
        }
    }
}

/// `ConnectList` stores mapping between IP-addresses and public keys.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConnectList {
    /// Peers to which we can connect.
    #[serde(default)]
    pub peers: BTreeMap<PublicKey, PeerAddress>,
}

impl ConnectList {
    /// Creates `ConnectList` from config.
    pub fn from_config(config: ConnectListConfig) -> Self {
        let peers: BTreeMap<PublicKey, PeerAddress> = config
            .peers
            .into_iter()
            .map(|peer| (peer.public_key, PeerAddress::new(peer.address)))
            .collect();

        ConnectList { peers }
    }

    /// Returns `true` if a peer with the given public key can connect.
    pub fn is_peer_allowed(&self, peer: &PublicKey) -> bool {
        self.peers.contains_key(peer)
    }

    /// Check if we allow to connect to `address`.
    pub fn is_address_allowed(&self, address: &str) -> bool {
        self.peers.values().any(|a| a.address == address)
    }

    /// Adds peer to the ConnectList.
    pub fn add(&mut self, peer: ConnectInfo) {
        self.peers
            .insert(peer.public_key, PeerAddress::new(peer.address));
    }

    /// Get public key corresponding to validator with `address`.
    pub fn find_key_by_resolved_address(&self, address: &SocketAddr) -> Option<&PublicKey> {
        self.peers
            .iter()
            .find(|(_, a)| a.resolved.as_ref() == Some(address))
            .map(|(p, _)| p)
    }

    /// Get public key corresponding to validator with `address`.
    pub fn find_key_by_unresolved_address(&self, address: &str) -> Option<&PublicKey> {
        self.peers
            .iter()
            .find(|(_, a)| a.address.as_str() == address)
            .map(|(p, _)| p)
    }

    /// Resolves network address and stores it in the `ConnectList`.
    pub fn resolve_and_cache_peer_address(&mut self, address: &str) -> Option<SocketAddr> {
        let key = *self.find_key_by_unresolved_address(address)?;
        let address = self.peers[&key].address.clone();
        let address = address.to_socket_addrs().ok()?.next();
        self.peers.get_mut(&key).unwrap().resolved = address;
        address
    }

    /// Returns cached resolved network address of the peer.
    pub fn get_resolved_peer_address(&self, address: &str) -> Option<SocketAddr> {
        let key = self.find_key_by_unresolved_address(address)?;
        self.peers[key].resolved
    }
}

#[cfg(test)]
mod test {
    use rand::{RngCore, SeedableRng, XorShiftRng};

    use super::ConnectList;
    use crypto::{gen_keypair, PublicKey, PUBLIC_KEY_LENGTH};
    use node::ConnectInfo;

    static VALIDATORS: [[u8; 16]; 2] = [[1; 16], [2; 16]];
    static REGULAR_PEERS: [u8; 16] = [3; 16];

    fn make_keys(source: [u8; 16], count: usize) -> Vec<PublicKey> {
        let mut rng = XorShiftRng::from_seed(source);
        (0..count)
            .into_iter()
            .map(|_| {
                let mut key = [0; PUBLIC_KEY_LENGTH];
                rng.fill_bytes(&mut key);
                PublicKey::from_slice(&key).unwrap()
            })
            .collect()
    }

    fn check_in_connect_list(
        connect_list: &ConnectList,
        keys: &[PublicKey],
        in_connect_list: &[usize],
        not_in_connect_list: &[usize],
    ) {
        for i in in_connect_list {
            assert_eq!(connect_list.is_peer_allowed(&keys[*i]), true);
        }
        for i in not_in_connect_list {
            assert_eq!(connect_list.is_peer_allowed(&keys[*i]), false);
        }
    }

    #[test]
    fn test_whitelist() {
        let regular = make_keys(REGULAR_PEERS, 4);
        let address = "127.0.0.1:80".to_owned();

        let mut connect_list = ConnectList::default();
        check_in_connect_list(&connect_list, &regular, &[], &[0, 1, 2, 3]);
        connect_list.add(ConnectInfo {
            public_key: regular[0],
            address: address.clone(),
        });
        check_in_connect_list(&connect_list, &regular, &[0], &[1, 2, 3]);
        connect_list.add(ConnectInfo {
            public_key: regular[2],
            address: address.clone(),
        });
        check_in_connect_list(&connect_list, &regular, &[0, 2], &[1, 3]);

        assert_eq!(connect_list.peers.len(), 2);
    }

    #[test]
    fn test_validators_in_whitelist() {
        let regular = make_keys(REGULAR_PEERS, 4);
        let validators = make_keys(VALIDATORS[0], 2);
        let mut connect_list = ConnectList::default();
        check_in_connect_list(&connect_list, &regular, &[], &[0, 1, 2, 3]);
        check_in_connect_list(&connect_list, &validators, &[], &[0, 1]);
        assert_eq!(connect_list.peers.len(), 0);

        add_to_connect_list(&mut connect_list, &validators);
        assert_eq!(connect_list.peers.len(), 2);
        check_in_connect_list(&connect_list, &regular, &[], &[0, 1, 2, 3]);
        check_in_connect_list(&connect_list, &validators, &[0, 1], &[]);
    }

    fn add_to_connect_list(connect_list: &mut ConnectList, peers: &[PublicKey]) {
        let address = "127.0.0.1:80".to_owned();
        for peer in peers {
            connect_list.add(ConnectInfo {
                public_key: *peer,
                address: address.clone(),
            })
        }
    }

    #[test]
    fn test_update_validators() {
        let validators0 = make_keys(VALIDATORS[0], 2);
        let validators1 = make_keys(VALIDATORS[1], 2);
        let mut connect_list = ConnectList::default();
        assert_eq!(connect_list.peers.len(), 0);
        add_to_connect_list(&mut connect_list, &validators0);
        assert_eq!(connect_list.peers.len(), 2);
        check_in_connect_list(&connect_list, &validators0, &[0, 1], &[]);
        check_in_connect_list(&connect_list, &validators1, &[], &[0, 1]);
        add_to_connect_list(&mut connect_list, &validators1);
        assert_eq!(connect_list.peers.len(), 4);
        check_in_connect_list(&connect_list, &validators0, &[0, 1], &[]);
        check_in_connect_list(&connect_list, &validators1, &[0, 1], &[]);
    }

    #[test]
    fn test_address_allowed() {
        let (public_key, _) = gen_keypair();
        let address = "127.0.0.1:80".to_owned();

        let mut connect_list = ConnectList::default();
        assert!(!connect_list.is_address_allowed(&address));

        connect_list.add(ConnectInfo {
            public_key,
            address: address.clone(),
        });
        assert!(connect_list.is_address_allowed(&address));
    }

}
