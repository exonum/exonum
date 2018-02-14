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

// spell-checker:ignore bpath, deser, precheck, repr

use serde::{Serialize, Serializer, Deserialize, Deserializer};

use crypto::{CryptoHash, Hash, HashStream};
use storage::StorageValue;
use super::key::{BitsRange, ChildKind, ProofMapKey, ProofPath, KEY_SIZE};
use super::node::BranchNode;

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
#[derive(Debug, Fail)]
pub enum MapProofError {
    /// Non-terminal node for a map consisting of a single node.
    #[fail(display = "non-terminal node as a single key in proof")]
    NonTerminalNode(ProofPath),

    /// One path in the proof is a prefix of another path.
    #[fail(display = "embedded paths in proof")]
    EmbeddedPaths {
        /// Prefix key
        prefix: ProofPath,
        /// Key containing the prefix
        path: ProofPath,
    },

    /// One path is mentioned several times in the proof.
    #[fail(display = "duplicate path in proof")]
    DuplicatePath(ProofPath),

    /// Entries in the proof are not ordered by increasing path.
    #[fail(display = "invalid path ordering")]
    InvalidOrdering(ProofPath, ProofPath),
}

// Used instead of `(ProofPath, Hash)` only for the purpose of clearer (de)serialization.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
struct MapProofEntry {
    path: ProofPath,
    hash: Hash,
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
/// [`check()`]. Prior to the `check` conversion, you may use `*unchecked` methods
/// to obtain information about the proof.
///
/// ```
/// # use exonum::storage::{Database, MemoryDB, StorageValue, MapProof, ProofMapIndex};
/// # use exonum::crypto::hash;
/// let mut fork = { let db = MemoryDB::new(); db.fork() };
/// let mut map = ProofMapIndex::new("index", &mut fork);
/// let (h1, h2, h3) = (hash(&[1]), hash(&[2]), hash(&[3]));
/// map.put(&h1, 100u32);
/// map.put(&h2, 200u32);
/// // Get the proof from the index
/// let proof = map.get_multiproof(vec![h1, h3]);
/// // Check the proof consistency
/// let checked_proof = proof.check().unwrap();
/// assert_eq!(checked_proof.entries(), vec![(&h1, &100u32)]);
/// assert_eq!(checked_proof.missing_keys(), vec![&h3]);
/// assert_eq!(checked_proof.hash(), map.root_hash());
/// ```
///
/// # JSON serialization
///
/// `MapProof` is serialized to JSON as an object with 2 array fields:
///
/// - `proof` is an array of `{ "path": ProofPath, "hash": Hash }` objects. The entries are sorted
///   by increasing [`ProofPath`], but client implementors should not rely on this if security
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
/// let (h1, h2) = (hash(&[1]), hash(&[2]));
/// map.put(&h1, 100u32);
/// map.put(&h2, 200u32);
///
/// let proof = map.get_proof(h2);
/// assert_eq!(
///     serde_json::to_value(&proof).unwrap(),
///     json!({
///         "proof": [ { "path": ProofPath::new(&h1), "hash": 100u32.hash() } ],
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
/// [`check()`]: #method.check
/// [`ProofPath`]: struct.ProofPath.html
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MapProof<K, V> {
    entries: Vec<OptionalEntry<K, V>>,
    proof: Vec<MapProofEntry>,
}

/// Version of `MapProof` obtained after verification.
///
/// See [`MapProof`] for an example of usage.
///
/// [`MapProof`]: struct.MapProof.html#workflow
#[derive(Debug)]
pub struct CheckedMapProof<K, V> {
    entries: Vec<(K, Option<V>)>,
    hash: Hash,
}

/// Computes the root hash of the Merkle Patricia tree backing the specified entries
/// in the map view.
///
/// The tree is not restored in full; instead, we add the paths to
/// the tree in their lexicographic order (i.e., according to the `PartialOrd` implementation
/// of `ProofPath`) and keep track of the rightmost nodes (the right contour) of the tree.
/// It is easy to see that adding paths in the lexicographic order means that only
/// the nodes in the right contour may be updated on each step. Further, on each step
/// zero or more nodes are evicted from the contour, and a single new node is
/// added to it.
///
/// `entries` are assumed to be sorted by the path in increasing order.
fn collect(entries: &[MapProofEntry]) -> Result<Hash, MapProofError> {
    fn common_prefix(x: &ProofPath, y: &ProofPath) -> ProofPath {
        x.prefix(x.common_prefix_len(y))
    }

    /// Calculates hash for an isolated node in the Merkle Patricia tree.
    fn hash_isolated_node(path: &ProofPath, h: &Hash) -> Hash {
        HashStream::new()
            .update(path.as_bytes())
            .update(h.as_ref())
            .hash()
    }

    fn hash_branch(left_child: &MapProofEntry, right_child: &MapProofEntry) -> Hash {
        let mut branch = BranchNode::empty();
        branch.set_child(ChildKind::Left, &left_child.path, &left_child.hash);
        branch.set_child(ChildKind::Right, &right_child.path, &right_child.hash);
        branch.hash()
    }

    /// Folds two last entries in a contour and replaces them with the folded entry.
    ///
    /// Returns an updated common prefix between two last entries in the contour.
    fn fold(contour: &mut Vec<MapProofEntry>, last_prefix: ProofPath) -> Option<ProofPath> {
        let last_entry = contour.pop().unwrap();
        let penultimate_entry = contour.pop().unwrap();

        contour.push(MapProofEntry {
            path: last_prefix,
            hash: hash_branch(&penultimate_entry, &last_entry),
        });

        if contour.len() > 1 {
            let penultimate_entry = contour[contour.len() - 2];
            Some(common_prefix(&penultimate_entry.path, &last_prefix))
        } else {
            None
        }
    }

    match entries.len() {
        0 => Ok(Hash::default()),

        1 => {
            if !entries[0].path.is_leaf() {
                Err(MapProofError::NonTerminalNode(entries[0].path))
            } else {
                Ok(hash_isolated_node(&entries[0].path, &entries[0].hash))
            }
        }

        _ => {
            let mut contour: Vec<MapProofEntry> = Vec::with_capacity(8);
            // invariant: equal to the common prefix of the 2 last nodes in the contour
            let mut last_prefix;

            {
                let (first_entry, second_entry) = (&entries[0], &entries[1]);
                last_prefix = common_prefix(&first_entry.path, &second_entry.path);
                contour.push(*first_entry);
                contour.push(*second_entry);
            }

            for entry in entries.iter().skip(2) {
                let new_prefix = common_prefix(&contour.last().unwrap().path, &entry.path);
                let new_prefix_len = new_prefix.len();

                while contour.len() > 1 && new_prefix_len < last_prefix.len() {
                    fold(&mut contour, last_prefix).map(|prefix| { last_prefix = prefix; });
                }

                contour.push(*entry);
                last_prefix = new_prefix;
            }

            while contour.len() > 1 {
                fold(&mut contour, last_prefix).map(|prefix| { last_prefix = prefix; });
            }

            Ok(contour[0].hash)
        }
    }
}

/// Builder for [`MapProof`]s.
///
/// This struct is rarely needs to be used explicitly (except for testing purposes). Instead,
/// `MapProof`s can be created using [`get_proof()`] and [`get_multiproof()`] methods, or
/// deserialized using `serde`.
///
/// [`MapProof`]: struct.MapProof.html
/// [`get_proof()`]: struct.ProofMapIndex.html#method.get_proof
/// [`get_multiproof()`]: struct.ProofMapIndex.html#method.get_multiproof
#[derive(Debug)]
pub(super) struct MapProofBuilder<K, V> {
    entries: Vec<OptionalEntry<K, V>>,
    proof: Vec<MapProofEntry>,
}

impl<K, V> MapProofBuilder<K, V> {
    /// Creates a new builder.
    pub fn new() -> Self {
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

    /// Adds a proof entry into the builder. The `path` must be greater than keys of
    /// all proof entries previously added to the proof.
    pub fn add_proof_entry(mut self, path: ProofPath, hash: Hash) -> Self {
        debug_assert!(self.proof.last().map_or(true, |last| last.path < path));

        self.proof.push(MapProofEntry { path, hash });
        self
    }

    /// Adds several proof entries into the builder. The `paths` must be greater than keys of
    /// all proof entries previously added to the proof and sorted in increasing order.
    pub fn add_proof_entries<I>(mut self, paths: I) -> Self
    where
        I: IntoIterator<Item = (ProofPath, Hash)>,
    {
        self.proof.extend(paths.into_iter().map(
            |(path, hash)| MapProofEntry { path, hash },
        ));
        debug_assert!(self.proof.windows(2).all(|w| w[0].path < w[1].path));
        self
    }

    /// Creates a [`MapProof`] from the builder.
    ///
    /// [`MapProof`]: struct.MapProof.html
    pub fn create(self) -> MapProof<K, V> {
        MapProof {
            entries: self.entries,
            proof: self.proof,
        }
    }
}

impl<K, V> MapProof<K, V> {
    /// Maps this proof to another type of keys and/or values.
    pub fn map<F, L, U>(self, map_fn: F) -> MapProof<L, U>
    where
        F: FnMut((K, Option<V>)) -> (L, Option<U>),
    {
        MapProof {
            entries: self.entries
                .into_iter()
                .map(OptionalEntry::into)
                .map(map_fn)
                .map(OptionalEntry::from)
                .collect(),
            proof: self.proof,
        }
    }

    /// Provides access to the proof part of the view. Useful mainly for debug purposes.
    pub fn proof_unchecked(&self) -> Vec<(ProofPath, Hash)> {
        self.proof
            .iter()
            .cloned()
            .map(|e| (e.path, e.hash))
            .collect()
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
    fn precheck(&self) -> Result<(), MapProofError> {
        use std::cmp::Ordering;
        use self::MapProofError::*;

        // Check that entries in proof are in increasing order
        for w in self.proof.windows(2) {
            let (prev_path, path) = (&w[0].path, &w[1].path);
            match prev_path.partial_cmp(path) {
                Some(Ordering::Less) => {
                    if path.starts_with(prev_path) {
                        return Err(EmbeddedPaths {
                            prefix: *prev_path,
                            path: *path,
                        });
                    }
                }
                Some(Ordering::Equal) => {
                    return Err(DuplicatePath(*path));
                }
                Some(Ordering::Greater) => {
                    return Err(InvalidOrdering(*prev_path, *path));
                }
                None => unreachable!("Incomparable keys in proof"),
            }
        }

        // Check that no entry has a prefix among the paths in the proof entries.
        // In order to do this, it suffices to locate the closest smaller path in the proof entries
        // and check only it.
        for e in &self.entries {
            let path = ProofPath::new(e.key());

            match self.proof.binary_search_by(|pe| {
                pe.path.partial_cmp(&path).expect(
                    "Incomparable paths in proof",
                )
            }) {
                Ok(_) => {
                    return Err(DuplicatePath(path));
                }

                Err(index) if index > 0 => {
                    let prev_path = &self.proof[index - 1].path;
                    if path.starts_with(prev_path) {
                        return Err(EmbeddedPaths {
                            prefix: *prev_path,
                            path,
                        });
                    }
                }

                _ => {}
            }
        }

        Ok(())
    }

    /// Consumes this proof producing a `CheckedMapProof` structure.
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
    /// let (h1, h2) = (hash(&[1]), hash(&[2]));
    /// map.put(&h1, 100u32);
    /// map.put(&h2, 200u32);
    ///
    /// let proof = map.get_proof(h2);
    /// let checked_proof = proof.check().unwrap();
    /// assert_eq!(checked_proof.entries(), vec![(&h2, &200u32)]);
    /// assert_eq!(checked_proof.hash(), map.root_hash());
    /// ```
    ///
    /// [`ProofMapIndex`]: struct.ProofMapIndex.html
    pub fn check(self) -> Result<CheckedMapProof<K, V>, MapProofError> {
        self.precheck()?;
        let (mut proof, entries) = (self.proof, self.entries);

        proof.extend(entries.iter().filter_map(|e| {
            e.as_kv().map(|(k, v)| {
                MapProofEntry {
                    path: ProofPath::new(k),
                    hash: v.hash(),
                }
            })
        }));
        // Rust docs state that in the case `self.proof` and `self.entries` are sorted
        // (which is the case for `MapProof`s returned by `ProofMapIndex.get_proof()`),
        // the sort is performed very quickly.
        proof.sort_unstable_by(|x, y| {
            x.path.partial_cmp(&y.path).expect(
                "Incorrectly formed paths supplied to MapProof; \
                 paths should have `start` field set to 0",
            )
        });

        collect(&proof).map(|h| {
            CheckedMapProof {
                entries: entries.into_iter().map(OptionalEntry::into).collect(),
                hash: h,
            }
        })
    }
}

impl<K, V> CheckedMapProof<K, V> {
    /// Retrieves references to keys that the proof shows as missing from the map.
    pub fn missing_keys(&self) -> Vec<&K> {
        self.entries
            .iter()
            .filter_map(|kv| match *kv {
                (ref key, None) => Some(key),
                _ => None,
            })
            .collect()
    }

    /// Retrieves references to key-value pairs that the proof shows as present in the map.
    pub fn entries(&self) -> Vec<(&K, &V)> {
        self.entries
            .iter()
            .filter_map(|kv| match *kv {
                (ref key, Some(ref value)) => Some((key, value)),
                _ => None,
            })
            .collect()
    }

    /// Retrieves references to existing and non-existing entries in the proof.
    /// Existing entries have `Some` value, non-existing have `None`.
    pub fn all_entries(&self) -> Vec<(&K, Option<&V>)> {
        self.entries
            .iter()
            .map(|&(ref k, ref v)| (k, v.as_ref()))
            .collect()
    }

    /// Returns a hash of the map that this proof is constructed for.
    pub fn hash(&self) -> Hash {
        self.hash
    }
}
