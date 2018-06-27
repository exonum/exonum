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

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::{
    key::{BitsRange, ChildKind, ProofMapKey, ProofPath, KEY_SIZE}, node::{BranchNode, Node},
};
use crypto::{CryptoHash, Hash, HashStream};
use storage::StorageValue;

// Expected size of the proof, in number of hashed entries.
const DEFAULT_PROOF_CAPACITY: usize = 8;

impl Serialize for ProofPath {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut repr = String::with_capacity(KEY_SIZE * 8);
        for index in 0..self.len() {
            match self.bit(index) {
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
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::{self, Unexpected, Visitor};
        use std::fmt;

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

                Ok(if len == 8 * KEY_SIZE {
                    ProofPath::new(&bytes)
                } else {
                    ProofPath::new(&bytes).prefix(len as u16)
                })
            }
        }

        deserializer.deserialize_str(ProofPathVisitor)
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
        /// Prefix key.
        prefix: ProofPath,
        /// Key containing the prefix.
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

/// View of a `ProofMapIndex`, i.e., a subset of its elements coupled with a *proof*,
/// which jointly allow to restore the `merkle_root()` of the index. Besides existing elements,
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
///
/// // Get the proof from the index
/// let proof = map.get_multiproof(vec![h1, h3]);
///
/// // Check the proof consistency
/// let checked_proof = proof.check().unwrap();
/// assert_eq!(checked_proof.entries(), vec![(&h1, &100u32)]);
/// assert_eq!(checked_proof.missing_keys(), vec![&h3]);
/// assert_eq!(checked_proof.merkle_root(), map.merkle_root());
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
///   which is asserted by the proof.
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
#[derive(Debug, Serialize, Deserialize)]
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
                    if let Some(prefix) = fold(&mut contour, last_prefix) {
                        last_prefix = prefix;
                    }
                }

                contour.push(*entry);
                last_prefix = new_prefix;
            }

            while contour.len() > 1 {
                if let Some(prefix) = fold(&mut contour, last_prefix) {
                    last_prefix = prefix;
                }
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
        self.proof.extend(
            paths
                .into_iter()
                .map(|(path, hash)| MapProofEntry { path, hash }),
        );
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
        use self::MapProofError::*;
        use std::cmp::Ordering;

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
                pe.path
                    .partial_cmp(&path)
                    .expect("Incomparable paths in proof")
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
    /// assert_eq!(checked_proof.merkle_root(), map.merkle_root());
    /// ```
    ///
    /// [`ProofMapIndex`]: struct.ProofMapIndex.html
    pub fn check(self) -> Result<CheckedMapProof<K, V>, MapProofError> {
        self.precheck()?;
        let (mut proof, entries) = (self.proof, self.entries);

        proof.extend(entries.iter().filter_map(|e| {
            e.as_kv().map(|(k, v)| MapProofEntry {
                path: ProofPath::new(k),
                hash: v.hash(),
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

        // This check is required as duplicate paths can be introduced by entries
        // (further, it's generally possible that two different entry keys lead to the same
        // `ProofPath`).
        for window in proof.windows(2) {
            if window[0].path == window[1].path {
                return Err(MapProofError::DuplicatePath(window[0].path));
            }
        }

        collect(&proof).map(|h| CheckedMapProof {
            entries: entries.into_iter().map(OptionalEntry::into).collect(),
            hash: h,
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
    pub fn merkle_root(&self) -> Hash {
        self.hash
    }
}

/// Creates a proof for a single key.
pub fn create_proof<K, V, F>(
    key: K,
    root_node: Option<(ProofPath, Node<V>)>,
    lookup: F,
) -> MapProof<K, V>
where
    K: ProofMapKey,
    V: StorageValue,
    F: Fn(&ProofPath) -> Node<V>,
{
    fn combine(
        mut left_hashes: Vec<(ProofPath, Hash)>,
        right_hashes: Vec<(ProofPath, Hash)>,
    ) -> Vec<(ProofPath, Hash)> {
        left_hashes.extend(right_hashes.into_iter().rev());
        left_hashes
    }

    let searched_path = ProofPath::new(&key);

    match root_node {
        Some((root_path, Node::Branch(root_branch))) => {
            let mut left_hashes = Vec::with_capacity(DEFAULT_PROOF_CAPACITY);
            let mut right_hashes = Vec::with_capacity(DEFAULT_PROOF_CAPACITY);

            // Currently visited branch and its key, respectively
            let (mut branch, mut node_path) = (root_branch, root_path);

            // Do at least one loop, even if the supplied key does not match the root key.
            // This is necessary to put both children of the root node into the proof
            // in this case.
            loop {
                // <256 by induction; `branch` is always a branch node, and `node_path`
                // is its key
                let next_height = node_path.len();
                let next_bit = searched_path.bit(next_height);
                node_path = branch.child_path(next_bit);

                let other_path_and_hash =
                    (branch.child_path(!next_bit), *branch.child_hash(!next_bit));
                match !next_bit {
                    ChildKind::Left => left_hashes.push(other_path_and_hash),
                    ChildKind::Right => right_hashes.push(other_path_and_hash),
                }

                if !searched_path.matches_from(&node_path, next_height) {
                    // Both children of `branch` do not fit

                    let next_hash = *branch.child_hash(next_bit);
                    match next_bit {
                        ChildKind::Left => left_hashes.push((node_path, next_hash)),
                        ChildKind::Right => right_hashes.push((node_path, next_hash)),
                    }

                    return MapProofBuilder::new()
                        .add_missing(key)
                        .add_proof_entries(combine(left_hashes, right_hashes))
                        .create();
                } else {
                    let node = lookup(&node_path);
                    match node {
                        Node::Branch(branch_) => branch = branch_,
                        Node::Leaf(value) => {
                            // We have reached the leaf node and haven't diverged!
                            // The key is there, we've just gotten the value, so we just
                            // need to return it.
                            return MapProofBuilder::new()
                                .add_entry(key, value)
                                .add_proof_entries(combine(left_hashes, right_hashes))
                                .create();
                        }
                    }
                }
            }
        }

        Some((root_path, Node::Leaf(root_value))) => {
            if root_path == searched_path {
                MapProofBuilder::new().add_entry(key, root_value).create()
            } else {
                MapProofBuilder::new()
                    .add_missing(key)
                    .add_proof_entry(root_path, root_value.hash())
                    .create()
            }
        }

        None => MapProofBuilder::new().add_missing(key).create(),
    }
}

/// Nodes in the contour during multiproof creation.
#[derive(Debug)]
struct ContourNode {
    key: ProofPath,
    branch: BranchNode,
    visited_left: bool,
    visited_right: bool,
}

impl ContourNode {
    fn new(key: ProofPath, branch: BranchNode) -> Self {
        ContourNode {
            key,
            branch,
            visited_left: false,
            visited_right: false,
        }
    }

    // Adds this contour node into a proof builder.
    fn add_to_proof<K, V>(self, mut builder: MapProofBuilder<K, V>) -> MapProofBuilder<K, V> {
        if !self.visited_right {
            // This works due to the following observation: If neither of the child nodes
            // were visited when the node is being ejected from the contour,
            // this means that it is safe to add the left and right hashes (in this order)
            // to the proof. The observation is provable by induction.
            if !self.visited_left {
                builder = builder.add_proof_entry(
                    self.branch.child_path(ChildKind::Left),
                    *self.branch.child_hash(ChildKind::Left),
                );
            }

            builder = builder.add_proof_entry(
                self.branch.child_path(ChildKind::Right),
                *self.branch.child_hash(ChildKind::Right),
            );
        }

        builder
    }
}

/// Processes a single key in a map with multiple entries.
fn process_key<K, V, F>(
    contour: &mut Vec<ContourNode>,
    mut builder: MapProofBuilder<K, V>,
    proof_path: &ProofPath,
    key: K,
    lookup: &F,
) -> MapProofBuilder<K, V>
where
    V: StorageValue,
    F: Fn(&ProofPath) -> Node<V>,
{
    // `unwrap()` is safe: there is at least 1 element in the contour by design
    let common_prefix = proof_path.common_prefix_len(&contour.last().unwrap().key);

    // Eject nodes from the contour while they will they can be "finalized"
    while let Some(node) = contour.pop() {
        if contour.is_empty() || node.key.len() <= common_prefix {
            contour.push(node);
            break;
        } else {
            builder = node.add_to_proof(builder);
        }
    }

    // Push new items to the contour
    'traverse: loop {
        let node_path = {
            let contour_tip = contour.last_mut().unwrap();

            let next_height = contour_tip.key.len();
            let next_bit = proof_path.bit(next_height);
            let node_path = contour_tip.branch.child_path(next_bit);

            if !proof_path.matches_from(&node_path, next_height) {
                // Both children of `branch` do not fit; stop here
                builder = builder.add_missing(key);
                break 'traverse;
            } else {
                match next_bit {
                    ChildKind::Left => contour_tip.visited_left = true,
                    ChildKind::Right => {
                        if !contour_tip.visited_left {
                            builder = builder.add_proof_entry(
                                contour_tip.branch.child_path(ChildKind::Left),
                                *contour_tip.branch.child_hash(ChildKind::Left),
                            );
                        }
                        contour_tip.visited_right = true;
                    }
                }

                node_path
            }
        };

        let node = lookup(&node_path);
        match node {
            Node::Branch(branch) => {
                contour.push(ContourNode::new(node_path, branch));
            }

            Node::Leaf(value) => {
                // We have reached the leaf node and haven't diverged!
                builder = builder.add_entry(key, value);
                break 'traverse;
            }
        }
    }

    builder
}

pub fn create_multiproof<K, V, KI, F>(
    keys: KI,
    root_node: Option<(ProofPath, Node<V>)>,
    lookup: F,
) -> MapProof<K, V>
where
    K: ProofMapKey,
    V: StorageValue,
    KI: IntoIterator<Item = K>,
    F: Fn(&ProofPath) -> Node<V>,
{
    match root_node {
        Some((root_path, Node::Branch(root_branch))) => {
            let mut builder = MapProofBuilder::new();

            let searched_paths = {
                let mut keys: Vec<_> = keys.into_iter().map(|k| (ProofPath::new(&k), k)).collect();

                keys.sort_unstable_by(|x, y| {
                    // `unwrap` is safe here because all keys start from the same position `0`
                    x.0.partial_cmp(&y.0).unwrap()
                });
                keys
            };

            let mut contour = Vec::with_capacity(DEFAULT_PROOF_CAPACITY);
            contour.push(ContourNode::new(root_path, root_branch));

            let mut last_searched_path: Option<ProofPath> = None;
            for (proof_path, key) in searched_paths {
                if last_searched_path == Some(proof_path) {
                    // The key has already been looked up; skipping.
                    continue;
                }

                builder = process_key(&mut contour, builder, &proof_path, key, &lookup);
                last_searched_path = Some(proof_path);
            }

            // Eject remaining entries from the contour
            while let Some(node) = contour.pop() {
                builder = node.add_to_proof(builder);
            }

            builder.create()
        }

        Some((root_path, Node::Leaf(root_value))) => {
            let mut builder = MapProofBuilder::new();
            // (One of) keys corresponding to the existing table entry.
            let mut found_key: Option<K> = None;

            for key in keys {
                let searched_path = ProofPath::new(&key);
                if root_path == searched_path {
                    found_key = Some(key);
                } else {
                    builder = builder.add_missing(key);
                }
            }

            builder = if let Some(key) = found_key {
                builder.add_entry(key, root_value)
            } else {
                builder.add_proof_entry(root_path, root_value.hash())
            };

            builder.create()
        }

        None => keys.into_iter()
            .fold(MapProofBuilder::new(), |builder, key| {
                builder.add_missing(key)
            })
            .create(),
    }
}
