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

pub use crate::ValidationError; // TODO Change for a type alias after EJB switching to rust > 1.36 (ECR-3827)

use exonum_crypto::Hash;
use failure::Fail;
use serde::{Deserializer, Serializer};
use serde_derive::{Deserialize, Serialize};

use std::{borrow::Cow, marker::PhantomData};

use super::{
    key::{BitsRange, ChildKind, ProofPath, KEY_SIZE},
    node::BranchNode,
};
use crate::{BinaryValue, HashTag, ObjectHash};

use crate::indexes::proof_map::key::{Hashed, ToProofPath};

impl serde::Serialize for ProofPath {
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

#[allow(clippy::use_self)]
impl<'de> serde::Deserialize<'de> for ProofPath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::{self, Unexpected, Visitor};
        use std::fmt;

        struct ProofPathVisitor;

        impl<'de> Visitor<'de> for ProofPathVisitor {
            type Value = ProofPath;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
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

                let mut bytes = [0_u8; KEY_SIZE];
                for (i, ch) in value.chars().enumerate() {
                    match ch {
                        '0' => {}
                        '1' => bytes[i / 8] += 1 << (i % 8),
                        _ => return Err(de::Error::invalid_value(Unexpected::Str(value), &self)),
                    }
                }

                Ok(if len == 8 * KEY_SIZE {
                    ProofPath::from_bytes(&bytes)
                } else {
                    ProofPath::from_bytes(&bytes).prefix(len as u16)
                })
            }
        }

        deserializer.deserialize_str(ProofPathVisitor)
    }
}

/// An error returned when a map proof is invalid.
///
/// This type is not intended to be exhaustively matched. It can be extended in the future
/// without breaking the semver compatibility.
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

    /// Never actually generated.
    #[doc(hidden)]
    #[fail(display = "")]
    __NonExhaustive,
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
        match self {
            OptionalEntry::Missing { missing } => missing,
            OptionalEntry::KV { key, .. } => key,
        }
    }

    fn as_missing(&self) -> Option<&K> {
        match self {
            OptionalEntry::Missing { missing } => Some(missing),
            _ => None,
        }
    }

    fn as_kv(&self) -> Option<(&K, &V)> {
        match self {
            OptionalEntry::KV { key, value } => Some((key, value)),
            _ => None,
        }
    }

    fn as_tuple(&self) -> (&K, Option<&V>) {
        match self {
            OptionalEntry::Missing { missing } => (missing, None),
            OptionalEntry::KV { key, value } => (key, Some(value)),
        }
    }
}

/// View of a `ProofMapIndex`, i.e., a subset of its elements coupled with a *proof*,
/// which jointly allow restoring the `object_hash()` of the index. Apart from the
/// existing elements, `MapProof` can assert absence of certain keys from the underlying
/// index.
///
/// # Workflow
///
/// You can create `MapProof`s with [`get_proof()`] and [`get_multiproof()`] methods of
/// `ProofMapIndex`. Proofs can be verified on the server side with the help of
/// [`check()`]. Prior to the `check` conversion, you may use `*unchecked` methods
/// to obtain information about the proof.
///
/// ```
/// # use exonum_merkledb::{
/// #     access::CopyAccessExt, Database, TemporaryDB, BinaryValue, MapProof, ObjectHash,
/// # };
/// # use exonum_crypto::hash;
/// # use failure::Error;
/// # fn main() -> Result<(), Error> {
/// let fork = { let db = TemporaryDB::new(); db.fork() };
/// let mut map = fork.get_proof_map("index");
/// let (h1, h2, h3) = (hash(&[1]), hash(&[2]), hash(&[3]));
/// map.put(&h1, 100u32);
/// map.put(&h2, 200u32);
///
/// // Get the proof from the index.
/// let proof = map.get_multiproof(vec![h1, h3]);
///
/// // Check the proof consistency.
/// let checked_proof = proof.check()?;
/// assert!(checked_proof.entries().eq(vec![(&h1, &100u32)]));
/// assert!(checked_proof.missing_keys().eq(vec![&h3]));
/// assert_eq!(checked_proof.index_hash(), map.object_hash());
///
/// // If the trusted list hash is known, there is a convenient method
/// // to combine integrity check and hash equality check.
/// let checked_proof = proof.check_against_hash(map.object_hash())?;
/// # Ok(())
/// # }
/// ```
///
/// # JSON serialization
///
/// `MapProof` is serialized to JSON as an object with 2 array fields:
///
/// - `proof` is an array of `{ path: ProofPath, hash: Hash }` objects.
/// - `entries` is an array with 2 kinds of objects: `{ missing: K }` for keys missing from
///   the underlying index, and `{ key: K, value: V }` for key-value pairs, existence of
///   which is asserted by the proof.
///
/// ```
/// # use serde_json::{self, json};
/// # use exonum_merkledb::{
/// #    access::CopyAccessExt, Database, TemporaryDB, BinaryValue, MapProof, HashTag,
/// #    proof_map::{Hashed, ToProofPath},
/// # };
/// # use exonum_crypto::hash;
/// let fork = { let db = TemporaryDB::new(); db.fork() };
/// let mut map = fork.get_proof_map("index");
/// let (h1, h2) = (HashTag::hash_leaf(&[1]), HashTag::hash_leaf(&[2]));
/// map.put(&h1, 100_u32);
/// map.put(&h2, 200_u32);
///
/// let proof = map.get_proof(h2);
/// assert_eq!(
///     serde_json::to_value(&proof).unwrap(),
///     json!({
///         "proof": [{
///             "path": Hashed::transform_key(&h1),
///             "hash": HashTag::hash_leaf(&100_u32.to_bytes()),
///         }],
///         "entries": [{ "key": h2, "value": 200 }],
///     })
/// );
/// ```
///
/// ## Note on external implementations
///
/// External implementations (e.g., in light clients) must treat serialized `MapProof`s
/// as untrusted inputs. Implementations may rely on the invariants provided by Exonum nodes
/// (e.g., ordering of `proof`; see [`check()`]) only if these invariants are checked
/// during proof verification.
///
/// [`get_proof()`]: struct.ProofMapIndex.html#method.get_proof
/// [`get_multiproof()`]: struct.ProofMapIndex.html#method.get_multiproof
/// [`check()`]: #method.check
/// [`ProofPath`]: struct.ProofPath.html
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MapProof<K, V, KeyMode = Hashed> {
    entries: Vec<OptionalEntry<K, V>>,
    proof: Vec<MapProofEntry>,
    #[serde(skip)]
    _key_mode: PhantomData<KeyMode>,
}

/// Version of `MapProof` obtained after verification.
///
/// See [`MapProof`] for an example of usage.
///
/// [`MapProof`]: struct.MapProof.html#workflow
#[derive(Debug, Serialize)]
pub struct CheckedMapProof<'a, K, V> {
    entries: &'a [OptionalEntry<K, V>],
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
fn collect(entries: &[Cow<'_, MapProofEntry>]) -> Result<Hash, MapProofError> {
    fn common_prefix(x: &ProofPath, y: &ProofPath) -> ProofPath {
        x.prefix(x.common_prefix_len(y))
    }

    fn hash_branch(left_child: &MapProofEntry, right_child: &MapProofEntry) -> Hash {
        let mut branch = BranchNode::empty();
        branch.set_child(ChildKind::Left, &left_child.path, &left_child.hash);
        branch.set_child(ChildKind::Right, &right_child.path, &right_child.hash);
        branch.object_hash()
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
            if entries[0].path.is_leaf() {
                Ok(HashTag::hash_single_entry_map(
                    &entries[0].path,
                    &entries[0].hash,
                ))
            } else {
                Err(MapProofError::NonTerminalNode(entries[0].path))
            }
        }

        _ => {
            let (first_entry, second_entry) = (&entries[0], &entries[1]);
            let mut contour: Vec<MapProofEntry> = vec![**first_entry, **second_entry];
            // invariant: equal to the common prefix of the 2 last nodes in the contour
            let mut last_prefix = common_prefix(&first_entry.path, &second_entry.path);

            for entry in entries.iter().skip(2) {
                let new_prefix = common_prefix(&contour.last().unwrap().path, &entry.path);
                let new_prefix_len = new_prefix.len();

                while contour.len() > 1 && new_prefix_len < last_prefix.len() {
                    if let Some(prefix) = fold(&mut contour, last_prefix) {
                        last_prefix = prefix;
                    }
                }

                contour.push(**entry);
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

impl<K, V, KeyMode> MapProof<K, V, KeyMode> {
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
    pub fn missing_keys_unchecked(&self) -> impl Iterator<Item = &K> {
        self.entries.iter().filter_map(OptionalEntry::as_missing)
    }

    /// Retrieves references to existing and non-existing entries in the proof.
    ///
    /// Existing entries have `Some` value, non-existing have `None`.
    /// This method does not perform any integrity checks of the proof.
    pub fn all_entries_unchecked(&self) -> impl Iterator<Item = (&K, Option<&V>)> {
        self.entries.iter().map(|e| match e {
            OptionalEntry::Missing { ref missing } => (missing, None),
            OptionalEntry::KV { ref key, ref value } => (key, Some(value)),
        })
    }

    /// Creates a new builder.
    pub(crate) fn new() -> Self {
        Self {
            entries: vec![],
            proof: vec![],
            _key_mode: PhantomData,
        }
    }

    /// Adds an existing entry into the builder.
    pub(crate) fn add_entry(mut self, key: K, value: V) -> Self {
        self.entries.push(OptionalEntry::value(key, value));
        self
    }

    /// Adds a missing key into the builder.
    pub(crate) fn add_missing(mut self, key: K) -> Self {
        self.entries.push(OptionalEntry::missing(key));
        self
    }

    /// Adds a proof entry into the builder. The `path` must be greater than keys of
    /// all proof entries previously added to the proof.
    pub(crate) fn add_proof_entry(mut self, path: ProofPath, hash: Hash) -> Self {
        debug_assert!(self.proof.last().map_or(true, |last| last.path < path));
        self.proof.push(MapProofEntry { path, hash });
        self
    }

    /// Adds several proof entries into the builder. The `paths` must be greater than keys of
    /// all proof entries previously added to the proof and sorted in increasing order.
    pub(crate) fn add_proof_entries<I>(mut self, paths: I) -> Self
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
}

impl<K, V, KeyMode> MapProof<K, V, KeyMode>
where
    V: BinaryValue,
    KeyMode: ToProofPath<K>,
{
    fn precheck(&self) -> Result<(), MapProofError> {
        use self::MapProofError::*;
        use std::cmp::Ordering;

        // Check that entries in `proof` are in increasing order.
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
            let path = KeyMode::transform_key(e.key());

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

    /// Checks this proof.
    ///
    /// ## Errors
    ///
    /// An error is returned if proof is malformed. The following checks are performed:
    ///
    /// - `proof` elements are ordered by increasing `path` field.
    /// - No path in `proof` is a prefix of another path in `proof` or a path inferred from
    ///   an entry.
    /// - Paths in `proof` and ones computed from `entries` are all distinct.
    ///
    /// # Examples
    ///
    /// ```
    /// # use exonum_merkledb::{access::CopyAccessExt, Database, TemporaryDB, ProofMapIndex, ObjectHash};
    /// # use exonum_crypto::hash;
    /// let fork = { let db = TemporaryDB::new(); db.fork() };
    /// let mut map = fork.get_proof_map("index");
    /// let (h1, h2) = (hash(&[1]), hash(&[2]));
    /// map.put(&h1, 100u32);
    /// map.put(&h2, 200u32);
    ///
    /// let proof = map.get_proof(h2);
    /// let checked_proof = proof.check().unwrap();
    /// assert_eq!(checked_proof.entries().collect::<Vec<_>>(), vec![(&h2, &200u32)]);
    /// assert_eq!(checked_proof.index_hash(), map.object_hash());
    /// ```
    ///
    /// [`ProofMapIndex`]: struct.ProofMapIndex.html
    pub fn check(&self) -> Result<CheckedMapProof<'_, K, V>, MapProofError> {
        self.precheck()?;

        let mut proof: Vec<_> = self.proof.iter().map(Cow::Borrowed).collect();
        proof.extend(self.entries.iter().filter_map(|e| {
            e.as_kv().map(|(k, v)| {
                Cow::Owned(MapProofEntry {
                    path: KeyMode::transform_key(&k),
                    hash: HashTag::hash_leaf(&v.to_bytes()),
                })
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

        collect(&proof).map(|merkle_root| CheckedMapProof {
            entries: &self.entries,
            hash: HashTag::hash_map_node(merkle_root),
        })
    }

    /// Checks this proof against a trusted map hash. Fails if the proof is malformed or the
    /// hash does not match the one computed from the proof.
    pub fn check_against_hash(
        &self,
        expected_map_hash: Hash,
    ) -> Result<CheckedMapProof<'_, K, V>, ValidationError<MapProofError>> {
        self.check()
            .map_err(ValidationError::Malformed)
            .and_then(|checked| {
                if checked.index_hash() == expected_map_hash {
                    Ok(checked)
                } else {
                    Err(ValidationError::UnmatchedRootHash)
                }
            })
    }
}

impl<'a, K, V> CheckedMapProof<'a, K, V> {
    /// Retrieves references to keys that the proof shows as missing from the map.
    pub fn missing_keys(&self) -> impl Iterator<Item = &'a K> {
        self.entries.iter().filter_map(OptionalEntry::as_missing)
    }

    /// Retrieves references to key-value pairs that the proof shows as present in the map.
    pub fn entries(&self) -> impl Iterator<Item = (&'a K, &'a V)> {
        self.entries.iter().filter_map(OptionalEntry::as_kv)
    }

    /// Retrieves references to existing and non-existing entries in the proof.
    /// Existing entries have `Some` value, non-existing have `None`.
    pub fn all_entries(&self) -> impl Iterator<Item = (&'a K, Option<&'a V>)> {
        self.entries.iter().map(OptionalEntry::as_tuple)
    }

    /// Returns the `object_hash()` of the underlying `ProofMapIndex`.
    pub fn index_hash(&self) -> Hash {
        self.hash
    }
}
