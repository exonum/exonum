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
    validators_list: BTreeSet<PublicKey>,
}



impl Whitelist {
    /// is this `peer` can connect or not
    pub fn allow(&self, peer: &PublicKey) -> bool {
        !self.whitelist_on || self.validators_list.contains(peer) ||
        self.whitelisted_peers.contains(peer)
    }

    /// append `peer` to whitelist
    pub fn add(&mut self, peer: PublicKey) {
        self.whitelisted_peers.insert(peer);
    }

    /// get list of peers in whitelist
    pub fn collect_allowed(&self) -> Vec<&PublicKey> {
        self.whitelisted_peers
            .iter()
            .chain(self.validators_list.iter())
            .collect()
    }

    pub fn set_validators<I>(&mut self, list: I)
        where I: IntoIterator<Item = PublicKey>
    {
        self.validators_list = list.into_iter().collect();
    }

    /// check if we support whitelist, or keep connection politics open
    /// if it return false, everybody can connect to us
    pub fn is_enabled(&self) -> bool {
        !self.whitelist_on
    }
}


#[cfg(test)]
mod test {
    use super::Whitelist;
    use crypto::PublicKey;

    static VALIDATORS: [[[u8; 32]; 2]; 2] = [[[1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                                               0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
                                              [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                                               0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2]],
                                             [[1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                                               0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1],
                                              [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                                               0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 2]]];
    static REGULAR_PEERS: [[u8; 32]; 4] = [[2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                                            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
                                           [2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                                            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2],
                                           [2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                                            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3],
                                           [2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                                            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4]];

    fn make_keys(source: &[[u8; 32]]) -> Vec<PublicKey> {
        source
            .iter()
            .map(|k| PublicKey::from_slice(k).unwrap())
            .collect()
    }

    fn check_in_whitelist(whitelist: &Whitelist,
                          keys: &[PublicKey],
                          in_whitelist: &[usize],
                          not_in_whitelist: &[usize]) {
        for i in in_whitelist {
            assert_eq!(whitelist.allow(&keys[*i]), true);
        }
        for i in not_in_whitelist {
            assert_eq!(whitelist.allow(&keys[*i]), false);
        }
    }

    #[test]
    fn test_whitelist() {
        let regular = make_keys(&REGULAR_PEERS);

        let mut whitelist = Whitelist::default();
        whitelist.whitelist_on = true;
        check_in_whitelist(&whitelist, &regular, &[], &[0, 1, 2, 3]);
        whitelist.add(regular[0]);
        check_in_whitelist(&whitelist, &regular, &[0], &[1, 2, 3]);
        whitelist.add(regular[2]);
        check_in_whitelist(&whitelist, &regular, &[0, 2], &[1, 3]);
        assert_eq!(whitelist.collect_allowed().len(), 2);
    }

    #[test]
    fn test_wildcard() {
        let regular = make_keys(&REGULAR_PEERS);

        let whitelist = Whitelist::default();
        check_in_whitelist(&whitelist, &regular, &[0, 1, 2, 3], &[]);
        assert_eq!(whitelist.collect_allowed().len(), 0);
    }

    #[test]
    fn test_validators_in_whitelist() {
        let regular = make_keys(&REGULAR_PEERS);
        let validators = make_keys(&VALIDATORS[0]);
        let mut whitelist = Whitelist::default();
        whitelist.whitelist_on = true;
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
        let validators0 = make_keys(&VALIDATORS[0]);
        let validators1 = make_keys(&VALIDATORS[1]);
        let mut whitelist = Whitelist::default();
        whitelist.whitelist_on = true;
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
