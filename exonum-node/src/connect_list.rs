// Copyright 2020 The Exonum Team
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

//! Mapping between peers public keys and IP addresses / domain names.

use exonum::{blockchain::ValidatorKeys, crypto::PublicKey};
use serde_derive::{Deserialize, Serialize};

use std::{collections::BTreeMap, fmt};

use crate::state::SharedConnectList;

#[cfg(test)]
use {crate::messages::Connect, exonum::messages::Verified};

/// Data needed to connect to a peer node.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ConnectInfo {
    /// Peer address.
    pub address: String,
    /// Peer public key.
    pub public_key: PublicKey,
}

impl fmt::Display for ConnectInfo {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.address)
    }
}

/// Stores mapping between IP addresses / domain names and public keys.
#[derive(Debug, Clone, Default)]
pub(crate) struct ConnectList {
    /// Peers to which we can connect.
    pub peers: BTreeMap<PublicKey, String>,
}

impl ConnectList {
    /// Creates `ConnectList` from config.
    pub fn from_config(config: ConnectListConfig) -> Self {
        let peers: BTreeMap<_, _> = config
            .peers
            .into_iter()
            .map(|peer| (peer.public_key, peer.address))
            .collect();

        Self { peers }
    }

    /// Creates `ConnectList` from the previously saved list of peers.
    #[cfg(test)]
    pub fn from_peers(peers: impl IntoIterator<Item = (PublicKey, Verified<Connect>)>) -> Self {
        Self {
            peers: peers
                .into_iter()
                .map(|(public_key, connect)| (public_key, connect.payload().host.clone()))
                .collect(),
        }
    }

    /// Returns `true` if a peer with the given public key can connect.
    pub(super) fn is_peer_allowed(&self, peer: &PublicKey) -> bool {
        self.peers.contains_key(peer)
    }

    /// Gets address of a peer with the specified public key.
    pub(super) fn find_address_by_pubkey(&self, key: &PublicKey) -> Option<&str> {
        self.peers.get(key).map(String::as_str)
    }

    /// Adds peer to the `ConnectList`.
    pub(crate) fn add(&mut self, peer: ConnectInfo) {
        self.peers.insert(peer.public_key, peer.address);
    }

    /// Updates peer address.
    pub(super) fn update_peer(&mut self, public_key: &PublicKey, address: String) {
        self.peers.insert(*public_key, address);
    }
}

/// Stores mapping between IP addresses / domain names and public keys.
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConnectListConfig {
    /// Peers to which the node knows how to connect.
    pub peers: Vec<ConnectInfo>,
}

impl ConnectListConfig {
    /// Creates `ConnectListConfig` from validators keys and corresponding IP addresses
    /// or domain names.
    pub fn from_validator_keys(validators_keys: &[ValidatorKeys], peers: &[String]) -> Self {
        let peers = peers
            .iter()
            .zip(validators_keys)
            .map(|(address, keys)| ConnectInfo {
                address: address.to_owned(),
                public_key: keys.consensus_key,
            })
            .collect();

        Self { peers }
    }

    /// Creates a `ConnectListConfig` from `ConnectList`.
    pub(super) fn from_connect_list(connect_list: &SharedConnectList) -> Self {
        Self {
            peers: connect_list.peers(),
        }
    }

    /// Returns peer addresses.
    pub(super) fn addresses(&self) -> Vec<String> {
        self.peers.iter().map(|p| p.address.clone()).collect()
    }
}

#[cfg(test)]
mod test {
    use exonum::crypto::{KeyPair, PublicKey, PUBLIC_KEY_LENGTH};
    use pretty_assertions::assert_eq;
    use rand::{rngs::StdRng, RngCore, SeedableRng};

    use super::*;
    use crate::ConnectInfo;

    const SEED_LENGTH: usize = 32;
    static VALIDATORS: [[u8; SEED_LENGTH]; 2] = [[1; SEED_LENGTH], [2; SEED_LENGTH]];
    static REGULAR_PEERS: [u8; SEED_LENGTH] = [3; SEED_LENGTH];

    fn make_keys(source: [u8; SEED_LENGTH], count: usize) -> Vec<PublicKey> {
        let mut rng: StdRng = SeedableRng::from_seed(source);
        (0..count)
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
            address,
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
        let public_key = KeyPair::random().public_key();
        let address = "127.0.0.1:80".to_owned();

        let mut connect_list = ConnectList::default();
        assert!(connect_list
            .peers
            .values()
            .all(|peer_addr| *peer_addr != address));

        connect_list.add(ConnectInfo {
            public_key,
            address: address.clone(),
        });
        assert!(connect_list
            .peers
            .values()
            .any(|peer_addr| *peer_addr == address));
    }
}
