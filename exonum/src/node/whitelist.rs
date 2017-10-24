// Copyright 2017 The Exonum Team
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

// TODO: don't reload whitelisted_peers if path the same (ECR-172)
use std::collections::BTreeSet;

use crypto::PublicKey;

/// `Whitelist` is special set to keep peers that can connect to us.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Whitelist {
    whitelist_enabled: bool,
    whitelisted_peers: BTreeSet<PublicKey>,

    #[serde(default)]
    #[serde(skip_serializing)]
    #[serde(skip_deserializing)]
    validators_list: BTreeSet<PublicKey>,
}

impl Whitelist {
    /// Returns `true` if a peer with the given public key can connect.
    pub fn allow(&self, peer: &PublicKey) -> bool {
        !self.whitelist_enabled || self.validators_list.contains(peer) ||
            self.whitelisted_peers.contains(peer)
    }

    /// Adds peer to the whitelist.
    pub fn add(&mut self, peer: PublicKey) {
        self.whitelisted_peers.insert(peer);
    }

    /// Returns list of whitelisted peers.
    pub fn collect_allowed(&self) -> Vec<&PublicKey> {
        self.whitelisted_peers
            .iter()
            .chain(self.validators_list.iter())
            .collect()
    }

    /// Resets list of validators with the given public keys.
    pub fn set_validators<I>(&mut self, list: I)
    where
        I: IntoIterator<Item = PublicKey>,
    {
        self.validators_list = list.into_iter().collect();
    }

    /// Returns `true` if whitelist is enabled, otherwise everyone can connect.
    pub fn is_enabled(&self) -> bool {
        self.whitelist_enabled
    }
}


#[cfg(test)]
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
            .map(|_| {
                PublicKey::from_slice(&<[u8; 32] as Rand>::rand(&mut rng)).unwrap()
            })
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
