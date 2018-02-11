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

use serde::{Serialize, Serializer, Deserialize, Deserializer};
use crypto::{Hash, HashStream};
use storage::StorageValue;
use super::key::{BitsRange, ChildKind, ProofMapKey, ProofPath, ProofPathPrefix, KEY_SIZE};

impl Serialize for ProofPath {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut repr = String::with_capacity(KEY_SIZE * 8);
        let bpath = self;
        for ind in 0..self.len() {
            match bpath.bit(ind) {
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

impl<'de> Deserialize<'de> for ProofPath {
    fn deserialize<D>(deser: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use std::fmt;
        use serde::de::{self, Visitor, Unexpected};

        struct ProofPathVisitor;

        impl<'de> Visitor<'de> for ProofPathVisitor {
            type Value = ProofPath;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                write!(
                    formatter,
                    "binary string with length between 1 and {}",
                    KEY_SIZE * 8
                )
            }

            fn visit_str<E>(self, value: &str) -> Result<ProofPath, E>
            where
                E: de::Error,
            {
                let len = value.len();
                if len == 0 || len > 8 * KEY_SIZE {
                    return Err(de::Error::invalid_value(Unexpected::Str(value), &self));
                }

                let mut bytes = [0u8; KEY_SIZE];
                for (i, ch) in value.chars().enumerate() {
                    match ch {
                        '0' => {}
                        '1' => bytes[i / 8] += 1 << (i % 8),
                        _ => return Err(de::Error::invalid_value(Unexpected::Str(value), &self)),
                    }
                }

                Ok(ProofPath::new(&bytes).prefix(len as u16))
            }
        }

        deser.deserialize_str(ProofPathVisitor)
    }
}

/// An error returned when a map proof is invalid.
#[derive(Debug)]
pub enum MapProofError {
    /// Non-terminal node for a map consisting of a single node.
    NonTerminalNode(ProofPath),

    /// One key in the proof is a prefix of another key.
    EmbeddedKeys {
        /// Prefix key
        prefix: ProofPath,
        /// Key containing the prefix
        key: ProofPath,
    },

    /// One key is mentioned several times in the proof.
    DuplicateKey(ProofPath),

    /// Entries in the proof are not ordered by increasing key.
    InvalidOrdering(ProofPath, ProofPath),
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
            NonTerminalNode(_) => "Non-terminal node as a single key in a map proof",
            EmbeddedKeys { .. } => "Embedded keys in a map proof",
            DuplicateKey(_) => "Duplicate keys in a map proof",
            InvalidOrdering(_, _) => "Invalid key ordering in a map proof",
        }
    }
}

#[derive(Debug)]
struct ContourNode<'a> {
    left_hash: Hash,
    left_key: ProofPathPrefix<'a>,
    key_len: u16,
    right_key_len: u16,
}

impl<'a> ContourNode<'a> {
    fn new(
        key_len: u16,
        left_key: ProofPathPrefix<'a>,
        left_hash: Hash,
        right_key_len: u16,
    ) -> Self {
        ContourNode {
            left_hash,
            left_key,
            key_len,
            right_key_len,
        }
    }

    #[inline]
    fn key_len(&self) -> u16 {
        self.key_len
    }

    #[inline]
    fn truncate_right_key(&mut self, to_bits: u16) {
        self.right_key_len = to_bits;
    }

    /// Outputs the hash of the node based on the finalized `right_hash` value and `contour_key`,
    /// which is an extension of the right child key.
    fn finalize(self, contour_key: &ProofPath, right_hash: Hash) -> Hash {
        let stream = HashStream::new().update(self.left_hash.as_ref()).update(
            right_hash.as_ref(),
        );
        let stream = self.left_key.hash_to(stream);
        let stream = contour_key.hashable_prefix(self.right_key_len).hash_to(
            stream,
        );

        stream.hash()
    }
}

// Used instead of `(ProofPath, Hash)` only for the purpose of clearer (de)serialization.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct MapProofEntry {
    key: ProofPath,
    hash: Hash,
}

impl From<(ProofPath, Hash)> for MapProofEntry {
    fn from(val: (ProofPath, Hash)) -> Self {
        MapProofEntry {
            key: val.0,
            hash: val.1,
        }
    }
}

impl From<MapProofEntry> for (ProofPath, Hash) {
    fn from(val: MapProofEntry) -> Self {
        (val.key, val.hash)
    }
}

// Used instead of `(K, Option<V>)` only for the purpose of clearer (de)serialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum OptionalEntry<K, V> {
    Missing { missing: K },
    KV { key: K, value: V },
}

impl<K, V> OptionalEntry<K, V> {
    fn missing(key: K) -> Self {
        OptionalEntry::Missing { missing: key }
    }

    fn value(key: K, value: V) -> Self {
        OptionalEntry::KV { key, value }
    }

    fn key(&self) -> &K {
        match *self {
            OptionalEntry::Missing { ref missing } => missing,
            OptionalEntry::KV { ref key, .. } => key,
        }
    }

    fn as_missing(&self) -> Option<&K> {
        match *self {
            OptionalEntry::Missing { ref missing } => Some(missing),
            _ => None,
        }
    }

    fn as_kv(&self) -> Option<(&K, &V)> {
        match *self {
            OptionalEntry::KV { ref key, ref value } => Some((key, value)),
            _ => None,
        }
    }

    fn into_kv(self) -> Option<(K, V)> {
        match self {
            OptionalEntry::KV { key, value } => Some((key, value)),
            _ => None,
        }
    }
}

impl<K, V> From<(K, Option<V>)> for OptionalEntry<K, V> {
    fn from(value: (K, Option<V>)) -> Self {
        match value {
            (missing, None) => OptionalEntry::Missing { missing },
            (key, Some(value)) => OptionalEntry::KV { key, value },
        }
    }
}

impl<K, V> Into<(K, Option<V>)> for OptionalEntry<K, V> {
    fn into(self) -> (K, Option<V>) {
        match self {
            OptionalEntry::Missing { missing } => (missing, None),
            OptionalEntry::KV { key, value } => (key, Some(value)),
        }
    }
}

/// View of a [`ProofMapIndex`], i.e., a subset of its elements coupled with a *proof*,
/// which jointly allow to restore the [`root_hash()`] of the index. Besides existing elements,
/// `MapProof` can assert absence of certain keys from the underlying index.
///
/// # Workflow
///
/// You can create `MapProof`s with [`get_proof()`] and [`get_multiproof()`] methods of
/// `ProofMapIndex`. Proofs can be verified on the server side with the help of
/// [`try_into()`]. Prior to the `try_into` conversion, you may use [`missing_keys()`]
/// to extract the keys missing from the underlying index.
///
/// ```
/// # use exonum::storage::{Database, MemoryDB, StorageValue, MapProof, ProofMapIndex};
/// # use exonum::crypto::hash;
/// let mut fork = { let db = MemoryDB::new(); db.fork() };
/// let mut map = ProofMapIndex::new("index", &mut fork);
/// let (h1, h2, h3) = (hash(&vec![1]), hash(&vec![2]), hash(&vec![3]));
/// map.put(&h1, 100u32);
/// map.put(&h2, 200u32);
/// // Get the proof from the index
/// let proof = map.get_multiproof(vec![h1, h3]);
/// // Check the missing elements
/// assert_eq!(proof.missing_keys().unwrap(), vec![&h3]);
/// // Check the proof consistency
/// assert_eq!(proof.try_into().unwrap(), (vec![(h1, 100u32)], map.root_hash()));
/// ```
///
/// # JSON serialization
///
/// `MapProof` is serialized to JSON as an object with 2 array fields:
///
/// - `proof` is an array of `{ "key": ProofPath, "hash": Hash }` objects. The entries are sorted by
///   increasing [`ProofPath`], but client implementors should not rely on this if security
///   is a concern.
/// - `entries` is an array with 2 kinds of objects: `{ "missing": K }` for keys missing from
///   the underlying index, and `{ "key": K, "value": V }` for key-value pairs, existence of
///   which is asserted by the proof
///
/// ```
/// # extern crate exonum;
/// # #[macro_use] extern crate serde_json;
/// # use exonum::storage::{Database, MemoryDB, StorageValue, MapProof, ProofMapIndex};
/// # use exonum::storage::proof_map_index::ProofPath;
/// # use exonum::crypto::{hash, CryptoHash};
/// # fn main() {
/// let mut fork = { let db = MemoryDB::new(); db.fork() };
/// let mut map = ProofMapIndex::new("index", &mut fork);
/// let (h1, h2) = (hash(&vec![1]), hash(&vec![2]));
/// map.put(&h1, 100u32);
/// map.put(&h2, 200u32);
///
/// let proof = map.get_proof(h2);
/// assert_eq!(
///     serde_json::to_value(&proof).unwrap(),
///     json!({
///         "proof": [ { "key": ProofPath::new(&h1), "hash": 100u32.hash() } ],
///         "entries": [ { "key": h2, "value": 200 } ]
///     })
/// );
/// # }
/// ```
///
/// [`ProofMapIndex`]: struct.ProofMapIndex.html
/// [`root_hash()`]: struct.ProofMapIndex.html#method.root_hash
/// [`get_proof()`]: struct.ProofMapIndex.html#method.get_proof
/// [`get_multiproof()`]: struct.ProofMapIndex.html#method.get_multiproof
/// [`try_into()`]: #method.try_into
/// [`missing_keys()`]: #method.missing_keys
/// [`ProofPath`]: struct.ProofPath.html
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MapProof<K, V> {
    entries: Vec<OptionalEntry<K, V>>,
    proof: Vec<MapProofEntry>,
}

/// Calculates hash for an isolated node in the Merkle Patricia tree.
fn hash_isolated_node(key: &ProofPath, h: &Hash) -> Hash {
    HashStream::new()
        .update(key.as_bytes())
        .update(h.as_ref())
        .hash()
}

/// Computes the root hash of the Merkle Patricia tree backing the specified entries
/// in the map view.
///
/// The tree is not restored in full; instead, we add the keys to
/// the tree in their lexicographic order (i.e., according to the `PartialOrd` implementation
/// of `ProofPath`) and keep track of the rightmost nodes (the right contour) of the tree.
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
            let mut right_contour: Vec<ContourNode> = Vec::with_capacity(8);

            for w in entries.windows(2) {
                let (prev, entry) = (&w[0], &w[1]);
                let common_prefix = entry.key.common_prefix_len(&prev.key);

                let mut fin_hash = prev.hash;
                let mut fin_key_len = prev.key.len();
                while let Some(mut node) = right_contour.pop() {
                    let len = node.key_len();
                    if len < common_prefix {
                        node.truncate_right_key(common_prefix);
                        right_contour.push(node);
                        break;
                    } else if len > 0 {
                        fin_key_len = node.key_len();
                        fin_hash = node.finalize(&prev.key, fin_hash);
                    } else {
                        // `len == 0` is a special case; the node will be reinserted
                        // to the contour, so the left child length should not be updated.
                    }
                }

                let node = ContourNode::new(
                    common_prefix, // key length
                    prev.key.hashable_prefix(fin_key_len), // left key
                    fin_hash, // left hash
                    entry.key.len(), // right key length
                );
                right_contour.push(node);
            }

            // Iteratively finalize all remaining nodes in the tree. This handles the special case
            // when all keys start with the same bit(s).
            let (mut fin_hash, fin_key) = {
                let last_entry = entries.last().unwrap();
                (last_entry.hash, &last_entry.key)
            };
            while let Some(node) = right_contour.pop() {
                fin_hash = node.finalize(fin_key, fin_hash);
            }
            Ok(fin_hash)
        }
    }
}

/// Builder for [`MapProof`]s.
///
/// This struct is rarely needs to be used explicitly (except for testing purposes). Instead,
/// `MapProof`s can be created using [`get_proof()`] and [`get_multiproof()`] methods, or
/// deserialized using `serde`.
///
/// # Examples
///
/// ```
/// # use exonum::storage::{MapProof, StorageValue};
/// # use exonum::storage::proof_map_index::ProofPath;
/// # use exonum::crypto::{hash, CryptoHash};
/// let (k1, k2, k3) = ((-1i32).hash(), 0i32.hash(), 1i32.hash());
/// let proof = MapProof::builder()
///     .add_proof_entry(ProofPath::new(&k1), hash("FOO".as_ref()))
///     .add_entry(k2, "BAR".to_string())
///     .add_missing(k3)
///     .create();
/// # drop(proof);
/// ```
///
/// [`MapProof`]: struct.MapProof.html
/// [`get_proof()`]: struct.ProofMapIndex.html#method.get_proof
/// [`get_multiproof()`]: struct.ProofMapIndex.html#method.get_multiproof
#[derive(Debug)]
pub struct MapProofBuilder<K, V> {
    entries: Vec<OptionalEntry<K, V>>,
    proof: Vec<(ProofPath, Hash)>,
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

    /// Adds a proof entry into the builder. The `key` must be greater than keys of
    /// all proof entries previously added to the proof.
    pub fn add_proof_entry(mut self, key: ProofPath, hash: Hash) -> Self {
        debug_assert!(if let Some(&(ref last_key, _)) = self.proof.last() {
            *last_key < key
        } else {
            true
        });

        self.proof.push((key, hash));
        self
    }

    /// Creates a [`MapProof`] from the builder.
    ///
    /// [`MapProof`]: struct.MapProof.html
    pub fn create(self) -> MapProof<K, V> {
        MapProof {
            entries: self.entries,
            proof: self.proof.into_iter().map(|e| e.into()).collect(),
        }
    }
}

impl<K, V> MapProof<K, V> {
    /// Creates a builder instance for the map proof.
    pub fn builder() -> MapProofBuilder<K, V> {
        MapProofBuilder::new()
    }

    /// Creates a proof for a single entry.
    #[doc(hidden)]
    pub(crate) fn for_entry<I>(entry: (K, V), proof: I) -> Self
    where
        I: IntoIterator<Item = (ProofPath, Hash)>,
    {
        MapProof {
            entries: vec![OptionalEntry::value(entry.0, entry.1)],
            proof: proof.into_iter().map(|e| e.into()).collect(),
        }
    }

    /// Creates a proof of absence of a key.
    #[doc(hidden)]
    pub(crate) fn for_absent_key<I>(key: K, proof: I) -> Self
    where
        I: IntoIterator<Item = (ProofPath, Hash)>,
    {
        MapProof {
            entries: vec![OptionalEntry::missing(key)],
            proof: proof.into_iter().map(|e| e.into()).collect(),
        }
    }

    /// Creates a proof for an empty map.
    #[doc(hidden)]
    pub(crate) fn for_empty_map<KI>(keys: KI) -> Self
    where
        KI: IntoIterator<Item = K>,
    {
        MapProof {
            entries: keys.into_iter().map(OptionalEntry::missing).collect(),
            proof: vec![],
        }
    }

    /// Maps this proof to another type of keys and/or values.
    pub fn map<F, L, U>(self, map_fn: F) -> MapProof<L, U>
    where
        F: FnMut((K, Option<V>)) -> (L, Option<U>),
    {
        MapProof {
            entries: self.entries
                .into_iter()
                .map(|e| e.into())
                .map(map_fn)
                .map(|e| e.into())
                .collect(),
            proof: self.proof,
        }
    }

    /// Provides access to the proof part of the view. Useful mainly for debug purposes.
    pub fn proof(&self) -> Vec<(ProofPath, Hash)> {
        self.proof.iter().cloned().map(|e| e.into()).collect()
    }

    /// Retrieves references to keys that the proof shows as missing from the map.
    /// This method does not perform any integrity checks of the proof.
    pub fn missing_keys_unchecked(&self) -> Vec<&K> {
        self.entries.iter().filter_map(|e| e.as_missing()).collect()
    }
}

impl<K, V> MapProof<K, V>
where
    K: ProofMapKey,
    V: StorageValue,
{
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
                Some(Ordering::Equal) => {
                    return Err(MapProofError::DuplicateKey(*key));
                }
                Some(Ordering::Greater) => {
                    return Err(MapProofError::InvalidOrdering(*prev_key, *key));
                }
                None => unreachable!("Incomparable keys in proof"),
            }
        }

        // Check that no entry has a prefix among the keys in the proof entries.
        // In order to do this, it suffices to locate the closest smaller key in the proof entries
        // and check only it.
        for e in &self.entries {
            let key = ProofPath::new(e.key());

            match self.proof.binary_search_by(|pe| {
                pe.key.partial_cmp(&key).expect(
                    "Incomparable keys in proof",
                )
            }) {
                Ok(_) => {
                    return Err(MapProofError::DuplicateKey(key));
                }

                Err(index) if index > 0 => {
                    let prev_key = &self.proof[index - 1].key;
                    if key.starts_with(prev_key) {
                        return Err(MapProofError::EmbeddedKeys {
                            prefix: *prev_key,
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
    /// Fails if the proof is malformed.
    pub fn missing_keys(&self) -> Result<Vec<&K>, MapProofError> {
        self.validate()?;
        Ok(self.missing_keys_unchecked())
    }

    /// Consumes this proof producing a pair of:
    ///
    /// - Collection from key-value pairs present in the proof
    /// - Hash of the [`ProofMapIndex`] that backs the proof
    ///
    /// Fails if the proof is malformed.
    ///
    /// # Examples
    ///
    /// ```
    /// # use exonum::storage::{Database, MemoryDB, ProofMapIndex};
    /// # use exonum::crypto::hash;
    /// let mut fork = { let db = MemoryDB::new(); db.fork() };
    /// let mut map = ProofMapIndex::new("index", &mut fork);
    /// let (h1, h2) = (hash(&vec![1]), hash(&vec![2]));
    /// map.put(&h1, 100u32);
    /// map.put(&h2, 200u32);
    ///
    /// let proof = map.get_proof(h2);
    /// assert_eq!(
    ///     proof.try_into().unwrap(),
    ///     (vec![(h2, 200u32)], map.root_hash())
    /// );
    /// ```
    ///
    /// [`ProofMapIndex`]: struct.ProofMapIndex.html
    pub fn try_into<T>(self) -> Result<(T, Hash), MapProofError>
    where
        T: FromIterator<(K, V)>,
    {
        self.validate()?;
        let (mut proof, entries) = (self.proof, self.entries);

        proof.extend(entries.iter().filter_map(|e| {
            e.as_kv().map(|(k, v)| (ProofPath::new(k), v.hash()).into())
        }));
        // Rust docs state that in the case `self.proof` and `self.entries` are sorted
        // (which is the case for `MapProof`s returned by `ProofMapIndex.get_proof()`),
        // the sort is performed very quickly.
        proof.sort_unstable_by(|x, y| {
            x.key.partial_cmp(&y.key).expect(
                "Incorrectly formed keys supplied to MapProof; \
                                              keys should have `from` field set to 0",
            )
        });

        collect(&proof).map(|h| {
            (
                entries
                    .into_iter()
                    .filter_map(OptionalEntry::into_kv)
                    .collect(),
                h,
            )
        })
    }
}
