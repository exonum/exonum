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

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::net::SocketAddr;

use blockchain::ValidatorKeys;
use crypto::PublicKey;
use helpers::fabric::NodePublicConfig;
use messages::Connect;
use node::ConnectInfo;

// TODO: Don't reload whitelisted_peers if path the same. (ECR-172)

/// `Whitelist` is mapping between IP addresses and public keys.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Whitelist {
    whitelist_enabled: bool,

    #[serde(default)]
    peers: BTreeMap<PublicKey, SocketAddr>,
}

impl Whitelist {
    /// Returns `true` if a peer with the given public key can connect.
    pub fn allow(&self, peer: &PublicKey) -> bool {
        !self.whitelist_enabled || self.peers.contains_key(peer)
    }

    /// Adds peer to the whitelist.
    pub fn add(&mut self, peer: ConnectInfo) {
        self.peers.insert(peer.public_key, peer.address);
    }

    /// Returns `true` if whitelist is enabled, otherwise everyone can connect.
    pub fn is_enabled(&self) -> bool {
        self.whitelist_enabled
    }

    /// Creates `Whitelist` from validators keys and corresponding IP addresses.
    pub fn from_validator_keys(validators_keys: &[ValidatorKeys], peers: &[SocketAddr]) -> Self {
        let peers: BTreeMap<PublicKey, SocketAddr> = peers
            .iter()
            .zip(validators_keys.iter())
            .map(|(p, v)| (v.consensus_key, *p))
            .collect();

        Whitelist {
            peers,
            whitelist_enabled: true,
        }
    }

    /// Creates `Whitelist` from validators public configs.
    pub fn from_node_config(list: &[NodePublicConfig]) -> Self {
        let peers: BTreeMap<PublicKey, SocketAddr> = list.iter()
            .map(|config| (config.validator_keys.consensus_key, config.addr))
            .collect();

        Whitelist {
            peers,
            whitelist_enabled: true,
        }
    }

    /// Check if we allow to connect to `address`.
    pub fn address_allowed(&self, address: &SocketAddr) -> bool {
        !self.whitelist_enabled || self.peers.values().any(|a| a == address)
    }

    /// Refresh whitelist peers if validators has changed.
    pub fn refresh(&mut self, validators_keys: &[ValidatorKeys]) {
        let keys: BTreeSet<_> = validators_keys
            .iter()
            .map(|key| key.consensus_key)
            .collect();
        self.peers = self.peers
            .clone()
            .into_iter()
            .filter(|(k, _)| keys.contains(k))
            .collect();
    }

    /// Get public key corresponding to validator with `address`.
    pub fn find_key_by_address(&self, address: &SocketAddr) -> Option<&PublicKey> {
        self.peers
            .iter()
            .find(|(p, a)| a == &address)
            .map(|(p, _)| p)
    }

    // Creates from state::peers, needed only for testing.
    #[doc(hidden)]
    pub fn from_peers_for_testing(peers: &HashMap<PublicKey, Connect>) -> Self {
        let whitelist = Whitelist::default();
        let peers: BTreeMap<PublicKey, SocketAddr> =
            peers.iter().map(|(p, c)| (*p, c.addr())).collect();
        Whitelist { peers, ..whitelist }
    }
}

#[cfg(test)]
mod test {
    use super::Whitelist;
    use blockchain::ValidatorKeys;
    use crypto::{gen_keypair, PublicKey};
    use std::collections::BTreeMap;
    use std::net::SocketAddr;

    #[test]
    fn test_whitelist_refresh() {
        let mut peers = BTreeMap::new();

        let (pk, sk) = gen_keypair();
        let addr: SocketAddr = "127.0.0.1:80".parse().unwrap();
        peers.insert(pk, addr.clone());

        let mut whitelist = Whitelist {
            peers,
            whitelist_enabled: true,
        };

        assert!(whitelist.allow(&pk));

        let mut validator_keys = Vec::new();
        validator_keys.push(ValidatorKeys {
            consensus_key: pk.clone(),
            service_key: pk.clone(),
        });
        whitelist.refresh(&validator_keys);
        assert!(whitelist.allow(&pk));

        let (pk, sk) = gen_keypair();
        validator_keys.push(ValidatorKeys {
            consensus_key: pk,
            service_key: pk,
        });
        whitelist.refresh(&validator_keys);
        assert!(!whitelist.allow(&pk));
    }
}

// TODO: rewrite tests
#[cfg(whitelist_tests)]
mod test {
    use super::Whitelist;
    use crypto::PublicKey;
    use rand::{Rand, SeedableRng, XorShiftRng};

    static VALIDATORS: [[u32; 4]; 2] = [[123, 45, 67, 89], [223, 45, 67, 98]];
    static REGULAR_PEERS: [u32; 4] = [5, 6, 7, 9];

    fn make_keys(source: [u32; 4], count: usize) -> Vec<PublicKey> {
        let mut rng = XorShiftRng::from_seed(source);
        (0..count)
            .into_iter()
            .map(|_| PublicKey::from_slice(&<[u8; 32] as Rand>::rand(&mut rng)).unwrap())
            .collect()
    }

    fn check_in_whitelist(
        whitelist: &Whitelist,
        keys: &[PublicKey],
        in_whitelist: &[usize],
        not_in_whitelist: &[usize],
    ) {
        for i in in_whitelist {
            assert_eq!(whitelist.allow(&keys[*i]), true);
        }
        for i in not_in_whitelist {
            assert_eq!(whitelist.allow(&keys[*i]), false);
        }
    }

    #[test]
    fn test_whitelist() {
        let regular = make_keys(REGULAR_PEERS, 4);

        let mut whitelist = Whitelist::default();
        whitelist.whitelist_enabled = true;
        check_in_whitelist(&whitelist, &regular, &[], &[0, 1, 2, 3]);
        whitelist.add(regular[0]);
        check_in_whitelist(&whitelist, &regular, &[0], &[1, 2, 3]);
        whitelist.add(regular[2]);
        check_in_whitelist(&whitelist, &regular, &[0, 2], &[1, 3]);
        assert_eq!(whitelist.collect_allowed().len(), 2);
    }

    #[test]
    fn test_wildcard() {
        let regular = make_keys(REGULAR_PEERS, 4);

        let mut whitelist = Whitelist::default();
        assert_eq!(whitelist.is_enabled(), false);
        check_in_whitelist(&whitelist, &regular, &[0, 1, 2, 3], &[]);
        whitelist.whitelist_enabled = true;
        assert_eq!(whitelist.is_enabled(), true);
        check_in_whitelist(&whitelist, &regular, &[], &[0, 1, 2, 3]);
        assert_eq!(whitelist.collect_allowed().len(), 0);
    }

    #[test]
    fn test_validators_in_whitelist() {
        let regular = make_keys(REGULAR_PEERS, 4);
        let validators = make_keys(VALIDATORS[0], 2);
        let mut whitelist = Whitelist::default();
        whitelist.whitelist_enabled = true;
        check_in_whitelist(&whitelist, &regular, &[], &[0, 1, 2, 3]);
        check_in_whitelist(&whitelist, &validators, &[], &[0, 1]);
        assert_eq!(whitelist.collect_allowed().len(), 0);
        whitelist.set_validators(validators.clone());
        assert_eq!(whitelist.collect_allowed().len(), 2);
        check_in_whitelist(&whitelist, &regular, &[], &[0, 1, 2, 3]);
        check_in_whitelist(&whitelist, &validators, &[0, 1], &[]);
    }

    #[test]
    fn test_update_validators() {
        let validators0 = make_keys(VALIDATORS[0], 2);
        let validators1 = make_keys(VALIDATORS[1], 2);
        let mut whitelist = Whitelist::default();
        whitelist.whitelist_enabled = true;
        assert_eq!(whitelist.collect_allowed().len(), 0);
        whitelist.set_validators(validators0.clone());
        assert_eq!(whitelist.collect_allowed().len(), 2);
        check_in_whitelist(&whitelist, &validators0, &[0, 1], &[]);
        check_in_whitelist(&whitelist, &validators1, &[], &[0, 1]);
        whitelist.set_validators(validators1.clone());
        assert_eq!(whitelist.collect_allowed().len(), 2);
        check_in_whitelist(&whitelist, &validators0, &[], &[0, 1]);
        check_in_whitelist(&whitelist, &validators1, &[0, 1], &[]);
    }
}
