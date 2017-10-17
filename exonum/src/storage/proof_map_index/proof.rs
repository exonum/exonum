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

use std::iter::FromIterator;

use serde::{Serialize, Serializer};
use crypto::{hash, Hash};

use super::super::StorageValue;
use super::key::{ProofMapKey, DBKey, ChildKind, KEY_SIZE};
use super::node::BranchNode;

impl Serialize for DBKey {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut repr = String::with_capacity(KEY_SIZE * 8);
        let bslice = self;
        for ind in 0..self.to() - self.from() {
            match bslice.get(ind) {
                ChildKind::Left => {
                    repr.push('0');
                }
                ChildKind::Right => {
                    repr.push('1');
                }
            }
        }
        ser.serialize_str(&repr)
    }
}

/// An error returned when a list proof is invalid.
#[derive(Debug)]
pub enum MapProofError {
    /// Non-terminal node for a map consisting of a single node.
    NonTerminalNode(DBKey),

    /// One key in the proof is a prefix of another key.
    EmbeddedKeys {
        /// Prefix key
        prefix: DBKey,
        /// Key containing the prefix
        key: DBKey,
    },

    /// One key is mentioned several times in the proof
    DuplicateKey(DBKey),

    /// Entries in the proof are not ordered by increasing key
    InvalidOrdering(DBKey, DBKey),
}

impl ::std::fmt::Display for MapProofError {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        write!(f, "{:?}", &self)
    }
}

impl ::std::error::Error for MapProofError {
    fn description(&self) -> &str {
        use self::MapProofError::*;

        match *self {
            NonTerminalNode(_) => &"Non-terminal node as a single key in a map proof",
            EmbeddedKeys { .. } => &"Embedded keys in a map proof",
            DuplicateKey(_) => &"Duplicate keys in a map proof",
            InvalidOrdering(_, _) => &"Invalid key ordering in a map proof",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
struct MapProofEntry {
    key: DBKey,
    hash: Hash,
}

impl From<(DBKey, Hash)> for MapProofEntry {
    fn from(val: (DBKey, Hash)) -> Self {
        MapProofEntry {
            key: val.0,
            hash: val.1,
        }
    }
}

impl From<MapProofEntry> for (DBKey, Hash) {
    fn from(val: MapProofEntry) -> Self {
        (val.key, val.hash)
    }
}

#[derive(Debug)]
struct ContourNode {
    key: Option<DBKey>,
    node: Option<BranchNode>,
    right_key: DBKey,
}

impl ContourNode {
    fn root(init_key: &DBKey) -> Self {
        ContourNode {
            key: None,
            node: None,
            right_key: *init_key,
        }
    }

    fn new(key: DBKey, left_key: DBKey, left_hash: Hash, right_key: DBKey) -> Self {
        ContourNode {
            key: Some(key),
            right_key,
            node: Some({
                let mut node = BranchNode::empty();
                node.set_child(ChildKind::Left, &left_key, &left_hash);
                node
            }),
        }
    }

    fn key_len(&self) -> u16 {
        match self.key {
            Some(ref key) => key.len(),
            None => 0,
        }
    }

    fn truncate_right_key(&mut self, to_bits: u16) {
        self.right_key.truncate_in_place(to_bits);
    }

    /// Outputs the hash and the key of the node based on the finalized `right_hash` value.
    fn finalize(self, right_hash: Hash) -> (DBKey, Hash) {
        if let (Some(key), Some(mut node)) = (self.key, self.node) {
            (key, {
                node.set_child(ChildKind::Right, &self.right_key, &right_hash);
                node.hash()
            })
        } else {
            (self.right_key, right_hash)
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct OptionalEntry<K, V>((K, Option<V>));

impl<K, V> OptionalEntry<K, V> {
    fn missing(key: K) -> Self {
        OptionalEntry((key, None))
    }

    fn value(key: K, value: V) -> Self {
        OptionalEntry((key, Some(value)))
    }

    fn key(&self) -> &K {
        &(self.0).0
    }

    fn get_missing(&self) -> Option<&K> {
        if (self.0).1.is_none() {
            Some(&(self.0).0)
        } else {
            None
        }
    }

    fn get_value(&self) -> Option<(&K, &V)> {
        let (key, value) = (&(self.0).0, &(self.0).1);
        if let &Some(ref v) = value {
            Some((key, v))
        } else {
            None
        }
    }

    fn into_value(self) -> Option<(K, V)> {
        let key = (self.0).0;
        (self.0).1.map(|value| (key, value))
    }
}

/// View of a `ProofMapIndex`, i.e., a subset of its elements coupled with a *proof*,
/// which jointly allow to restore the `root_hash()` of the index.
#[derive(Debug, Serialize)]
pub struct MapProof<K, V> {
    entries: Vec<OptionalEntry<K, V>>,
    proof: Vec<MapProofEntry>,
}

/// Calculates hash for an isolated node in the Merkle Patricia tree.
fn hash_isolated_node(key: &DBKey, h: &Hash) -> Hash {
    hash(&[&key.to_vec(), h.as_ref()].concat())
}

/// Computes the root hash of the Merkle Patricia tree backing the specified entries
/// in the map view.
///
/// The tree is not restored in full; instead, we add the keys to
/// the tree in their lexicographic order (i.e., according to the `PartialOrd` implementation
/// of `DBKey`) and keep track of the rightmost nodes (the right contour) of the tree.
/// It is easy to see that adding keys in the lexicographic order means that only
/// the nodes in the right contour may be updated on each step. Further, on each step
/// zero or more nodes are evicted from the contour, and a single new node is
/// added to it.
///
/// `entries` are assumed to be sorted by the key in increasing order.
fn collect(entries: &[MapProofEntry]) -> Result<Hash, MapProofError> {
    match entries.len() {
        0 => Ok(Hash::default()),

        1 => {
            if !entries[0].key.is_leaf() {
                Err(MapProofError::NonTerminalNode(entries[0].key))
            } else {
                Ok(hash_isolated_node(&entries[0].key, &entries[0].hash))
            }
        }

        _ => {
            let root = ContourNode::root(&entries[0].key);
            let mut right_contour = vec![root];

            for w in entries.windows(2) {
                let (prev, entry) = (&w[0], &w[1]);
                let common_prefix = entry.key.common_prefix(&prev.key);

                let mut fin_key_and_hash = (prev.key, prev.hash);
                while let Some(mut node) = right_contour.pop() {
                    if node.key_len() < common_prefix {
                        node.truncate_right_key(common_prefix);
                        right_contour.push(node);
                        break;
                    } else {
                        fin_key_and_hash = node.finalize(fin_key_and_hash.1);
                    }
                }

                let node = ContourNode::new(
                    entry.key.truncate(common_prefix), // key
                    fin_key_and_hash.0, // left key
                    fin_key_and_hash.1, // left hash
                    entry.key, // right key
                );
                right_contour.push(node);
            }

            // Iteratively finalize all remaining nodes in the tree. This handles the special case
            // when all keys start with the same bit(s); see the special clause in
            // `ContourNode.finalize()`.
            let mut fin_hash = entries.last().unwrap().hash;
            while let Some(node) = right_contour.pop() {
                fin_hash = node.finalize(fin_hash).1;
            }
            Ok(fin_hash)
        }
    }
}

/// Builder for `MapProof`s.
#[derive(Debug)]
pub struct MapProofBuilder<K, V> {
    entries: Vec<OptionalEntry<K, V>>,
    proof: Vec<(DBKey, Hash)>,
}

impl<K, V> MapProofBuilder<K, V> {
    /// Creates a new builder.
    fn new() -> Self {
        MapProofBuilder {
            entries: vec![],
            proof: vec![],
        }
    }

    /// Adds an existing entry into the builder.
    pub fn add_entry(mut self, key: K, value: V) -> Self {
        self.entries.push(OptionalEntry::value(key, value));
        self
    }

    /// Adds a missing key into the builder.
    pub fn add_missing(mut self, key: K) -> Self {
        self.entries.push(OptionalEntry::missing(key));
        self
    }

    /// Adds a proof entry into the builder.
    pub fn add_proof_entry(mut self, key: DBKey, hash: Hash) -> Self {
        self.proof.push((key, hash));
        self
    }

    /// Creates a `MapProof` from the builder.
    pub fn create(self) -> MapProof<K, V> {
        MapProof {
            entries: self.entries,
            proof: self.proof.into_iter().map(|e| e.into()).collect(),
        }
    }
}

impl<K, V> MapProof<K, V>
where
    K: ProofMapKey,
    V: StorageValue,
{
    /// Creates a builder instance for the map proof.
    pub fn builder() -> MapProofBuilder<K, V> {
        MapProofBuilder::new()
    }

    /// Creates a proof for a single entry.
    pub fn for_entry<I>(entry: (K, V), proof: I) -> Self
    where
        I: IntoIterator<Item = (DBKey, Hash)>,
    {
        MapProof {
            entries: vec![OptionalEntry::value(entry.0, entry.1)],
            proof: proof.into_iter().map(|e| e.into()).collect(),
        }
    }

    /// Creates a proof of absence of a key.
    pub fn for_absent_key<I>(key: K, proof: I) -> Self
    where
        I: IntoIterator<Item = (DBKey, Hash)>,
    {
        MapProof {
            entries: vec![OptionalEntry::missing(key)],
            proof: proof.into_iter().map(|e| e.into()).collect(),
        }
    }

    /// Creates a proof for an empty map.
    pub fn for_empty_map<KI>(keys: KI) -> Self
    where
        KI: IntoIterator<Item = K>,
    {
        MapProof {
            entries: keys.into_iter().map(|key| OptionalEntry::missing(key)).collect(),
            proof: vec![],
        }
    }

    /// Provides access to the proof part of the view. Useful mainly for debug purposes.
    pub fn proof(&self) -> Vec<(DBKey, Hash)> {
        self.proof.iter().cloned().map(|e| e.into()).collect()
    }

    fn validate(&self) -> Result<(), MapProofError> {
        use std::cmp::Ordering;

        // Check that entries in proof are in increasing order
        for w in self.proof.windows(2) {
            let (prev_key, key) = (&w[0].key, &w[1].key);
            match prev_key.partial_cmp(key) {
                Some(Ordering::Less) => {
                    if key.starts_with(prev_key) {
                        return Err(MapProofError::EmbeddedKeys {
                            prefix: *prev_key,
                            key: *key,
                        });
                    }
                }
                Some(Ordering::Equal) => { return Err(MapProofError::DuplicateKey(*key)); }
                Some(Ordering::Greater) => { return Err(MapProofError::InvalidOrdering(*prev_key, *key)); }
                None => unreachable!("Uncomparable keys in proof"),
            }
        }

        // Check that no entry has a prefix among the keys in the proof entries.
        // In order to do this, it suffices to locate the closest smaller key in the proof entries
        // and check only it.
        for e in &self.entries {
            let key = DBKey::leaf(e.key());

            match self.proof.binary_search_by(|pe| pe.key.partial_cmp(&key).expect("Uncomparable keys in proof")) {
                Ok(_) => { return Err(MapProofError::DuplicateKey(key)); }

                Err(index) if index > 0 => {
                    let prev_key = self.proof[index - 1].key;

                    if key.starts_with(&prev_key) {
                        return Err(MapProofError::EmbeddedKeys {
                            prefix: prev_key,
                            key,
                        });
                    }
                }

                _ => {}
            }
        }

        Ok(())
    }

    /// Retrieves references to keys that the proof shows as missing from the map.
    /// This method does not perform any integrity checks of the proof.
    pub fn missing_keys_unchecked<'a>(&'a self) -> Vec<&'a K> {
        self.entries.iter()
            .filter_map(|e| e.get_missing())
            .collect()
    }

    /// Retrieves references to keys that the proof shows as missing from the map.
    /// Fails if the proof is malformed.
    pub fn missing_keys<'a>(&'a self) -> Result<Vec<&'a K>, MapProofError> {
        self.validate()?;
        Ok(self.missing_keys_unchecked())
    }

    /// Consumes this view producing a pair of:
    /// - Collection from key-value pairs present in the view
    /// - Hash of the `ProofMapIndex` that backs the view
    ///
    /// Fails if the view is malformed (e.g., invalid proof).
    pub fn try_into<T>(self) -> Result<(T, Hash), MapProofError>
    where
        T: FromIterator<(K, V)>,
    {
        self.validate()?;
        let (mut proof, entries) = (self.proof, self.entries);

        proof.extend(entries.iter().filter_map(|e| {
            e.get_value().map(|(k, v)| (DBKey::leaf(k), v.hash()).into())
        }));
        // Rust docs state that in the case `self.proof` and `self.entries` are sorted
        // (which is the case for `MapProof`s returned by `ProofMapIndex.get_proof()`),
        // the sort is performed very quickly.
        proof.sort_by(|x, y| {
            x.key.partial_cmp(&y.key).expect(
                "Incorrectly formed keys supplied to MapProof; \
                                              keys should have `from` field set to 0",
            )
        });

        collect(&proof).map(|h| (entries.into_iter().filter_map(OptionalEntry::into_value).collect(), h))
    }
}
