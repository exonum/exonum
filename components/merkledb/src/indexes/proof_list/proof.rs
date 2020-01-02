// Copyright 2019 The Exonum Team
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

pub use crate::ValidationError; // TODO Change for a type alias after EJB switching to rust > 1.36

use exonum_crypto::Hash;
use failure::Fail;
use serde_derive::*;

use std::cmp::Ordering;

use super::{key::ProofListKey, tree_height_by_length};
use crate::{BinaryValue, HashTag};

#[cfg(feature = "with-protobuf")]
use crate::{proto, ProtobufConvert};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "with-protobuf", derive(ProtobufConvert))]
#[cfg_attr(
    feature = "with-protobuf",
    protobuf_convert(source = "proto::list_proof::HashedEntry")
)]
pub struct HashedEntry {
    #[serde(flatten)]
    key: ProofListKey,
    hash: Hash,
}

impl HashedEntry {
    pub fn new(key: ProofListKey, hash: Hash) -> Self {
        Self { key, hash }
    }
}

/// View of a `ProofListIndex`, i.e., a subset of its elements coupled with a *proof*,
/// which jointly allow restoring the `object_hash()` of the index. Apart from proving
/// elements in the list, `ListProof` can assert that the list is shorter than the requested
/// range of indexes.
///
/// # Workflow
///
/// You can create `ListProof`s with [`get_proof()`] and [`get_range_proof()`] methods of
/// `ProofListIndex`. Proofs can be verified on the server side with the help of
/// [`check()`]. Prior to the `check` conversion, you may use `*unchecked` methods
/// to obtain information about the proof.
///
/// ```
/// # use exonum_merkledb::{
/// #     access::AccessExt, Database, TemporaryDB, BinaryValue, ListProof, ObjectHash,
/// # };
/// # use failure::Error;
/// # fn main() -> Result<(), Error> {
/// let fork = { let db = TemporaryDB::new(); db.fork() };
/// let mut list = fork.get_proof_list("index");
/// list.extend(vec![100_u32, 200_u32, 300_u32]);
///
/// // Get the proof from the index
/// let proof = list.get_range_proof(1..);
///
/// // Check the proof consistency.
/// let checked_proof = proof.check()?;
/// assert_eq!(checked_proof.index_hash(), list.object_hash());
/// assert_eq!(*checked_proof.entries(), [(1, 200_u32), (2, 300_u32)]);
///
/// // If the trusted list hash is known, there is a convenient method
/// // to combine integrity check and hash equality check.
/// let checked_proof = proof.check_against_hash(list.object_hash())?;
/// assert!(checked_proof.indexes().eq(1..=2));
/// # Ok(())
/// # }
/// ```
///
/// # JSON serialization
///
/// `ListProof` is serialized to JSON as an object with the following fields:
///
/// - `proof` is an array of `{ height: number, index: number, hash: Hash }` objects.
/// - `entries` is an array with list elements and their indexes, that is,
///   tuples `[number, V]`.
/// - `length` is the length of the underlying `ProofListIndex`.
///
/// ```
/// # use serde_json::{self, json};
/// # use exonum_merkledb::{
/// #     access::AccessExt, Database, TemporaryDB, BinaryValue, HashTag, ListProof,
/// # };
/// # fn main() {
/// let fork = { let db = TemporaryDB::new(); db.fork() };
/// let mut list = fork.get_proof_list("index");
/// list.extend(vec![1_u32, 2, 3]);
/// let h1 = HashTag::hash_leaf(&1_u32.to_bytes());
/// let h3 = HashTag::hash_leaf(&3_u32.to_bytes());
/// let h33 = HashTag::hash_single_node(&h3);
///
/// let proof = list.get_proof(1);
/// assert_eq!(
///     serde_json::to_value(&proof).unwrap(),
///     json!({
///         "proof": [
///             { "index": 0, "height": 1, "hash": h1 },
///             { "index": 1, "height": 2, "hash": h33 },
///         ],
///         "entries": [[1, 2]],
///         "length": 3,
///     })
/// );
/// # }
/// ```
///
/// ## Note on external implementations
///
/// External implementations (e.g., in light clients) must treat serialized `ListProof`s
/// as untrusted inputs. Implementations may rely on the invariants provided by Exonum nodes
/// (e.g., ordering of `proof` / `entries`; see [`check()`]) only if these invariants are checked
/// during proof verification.
///
/// [`get_proof()`]: struct.ProofListIndex.html#method.get_proof
/// [`get_range_proof()`]: struct.ProofListIndex.html#method.get_range_proof
/// [`check()`]: #method.check
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct ListProof<V> {
    proof: Vec<HashedEntry>,
    entries: Vec<(u64, V)>,
    length: u64,
}

/// Merges two iterators with `HashedEntry`s so that the elements in the resulting iterator
/// are ordered by increasing `HashedEntry.key`.
///
/// # Arguments
///
/// Both inputs need to be ordered by `HashedEntry.key`.
///
/// # Return value
///
/// Iterator will yield an error if there is an equal `HashedEntry.key` present in both
/// input iterators.
fn merge(
    first: impl Iterator<Item = HashedEntry>,
    second: impl Iterator<Item = HashedEntry>,
) -> impl Iterator<Item = Result<HashedEntry, ()>> {
    struct Merge<T, U> {
        first: T,
        second: U,
        first_item: Option<HashedEntry>,
        second_item: Option<HashedEntry>,
    }

    impl<T, U> Merge<T, U>
    where
        T: Iterator<Item = HashedEntry>,
        U: Iterator<Item = HashedEntry>,
    {
        fn new(mut first: T, mut second: U) -> Self {
            let (first_item, second_item) = (first.next(), second.next());
            Self {
                first,
                second,
                first_item,
                second_item,
            }
        }
    }

    impl<T, U> Iterator for Merge<T, U>
    where
        T: Iterator<Item = HashedEntry>,
        U: Iterator<Item = HashedEntry>,
    {
        type Item = Result<HashedEntry, ()>;

        fn next(&mut self) -> Option<Self::Item> {
            match (self.first_item, self.second_item) {
                (Some(x), Some(y)) => match x.key.cmp(&y.key) {
                    Ordering::Less => {
                        self.first_item = self.first.next();
                        Some(Ok(x))
                    }
                    Ordering::Greater => {
                        self.second_item = self.second.next();
                        Some(Ok(y))
                    }
                    Ordering::Equal => Some(Err(())),
                },

                (Some(x), None) => {
                    self.first_item = self.first.next();
                    Some(Ok(x))
                }

                (None, Some(y)) => {
                    self.second_item = self.second.next();
                    Some(Ok(y))
                }

                (None, None) => None,
            }
        }
    }

    Merge::new(first, second)
}

/// Takes a subset of hashes at a particular height in the Merkle tree and
/// computes all known hashes on the next height.
///
/// # Arguments
///
/// - `last_index` is the index of the last element in the Merkle tree on the given height.
///
/// # Return value
///
/// The `layer` is modified in place. An error is returned if the layer is malformed (e.g.,
/// there is insufficient data to hash it).
///
/// # Examples
///
/// See unit tests at the end of this file.
fn hash_layer(layer: &mut Vec<HashedEntry>, last_index: u64) -> Result<(), ListProofError> {
    let new_len = (layer.len() + 1) / 2;
    for i in 0..new_len {
        let x = &layer[2 * i];
        layer[i] = if let Some(y) = layer.get(2 * i + 1) {
            // To be able to zip two hashes on the layer, they need to be adjacent to each other,
            // and the first of them needs to have an even index.
            if !x.key.is_left() || y.key.index() != x.key.index() + 1 {
                return Err(ListProofError::MissingHash);
            }
            HashedEntry::new(x.key.parent(), HashTag::hash_node(&x.hash, &y.hash))
        } else {
            // If there is an odd number of hashes on the layer, the solitary hash must have
            // the greatest possible index.
            if last_index % 2 == 1 || x.key.index() != last_index {
                return Err(ListProofError::MissingHash);
            }
            HashedEntry::new(x.key.parent(), HashTag::hash_single_node(&x.hash))
        };
    }

    layer.truncate(new_len);
    Ok(())
}

impl<V: BinaryValue> ListProof<V> {
    pub(super) fn new<I>(values: I, length: u64) -> Self
    where
        I: IntoIterator<Item = (u64, V)>,
    {
        Self {
            entries: values.into_iter().collect(),
            proof: vec![],
            length,
        }
    }

    pub(super) fn empty(merkle_root: Hash, length: u64) -> Self {
        let proof = if length == 0 {
            // The empty tree is special: it does not require the root element in the proof.
            vec![]
        } else {
            let height = tree_height_by_length(length);
            vec![HashedEntry {
                key: ProofListKey::new(height, 0),
                hash: merkle_root,
            }]
        };

        Self {
            entries: vec![],
            proof,
            length,
        }
    }

    pub(super) fn push_hash(&mut self, height: u8, index: u64, hash: Hash) -> &mut Self {
        debug_assert!(height > 0);

        let key = ProofListKey::new(height, index);
        debug_assert!(
            if let Some(&HashedEntry { key: last_key, .. }) = self.proof.last() {
                key > last_key
            } else {
                true
            }
        );

        self.proof.push(HashedEntry::new(key, hash));
        self
    }

    /// Restores the root hash of the Merkle tree.
    ///
    /// The root hash is computed by iterating over each height of the Merkle tree
    /// and computing hashes on this height based on the information in the proof.
    /// We don't need to restore *all* hashes on *all* heights; we just need sufficient information
    /// to restore the single hash at the last height (which is the Merkle tree root).
    ///
    /// For proofs of a single element or a contiguous range of elements,
    /// the total number of restored hashes is `O(log_2(N))`, where `N` is the list length.
    fn collect(&self) -> Result<Hash, ListProofError> {
        let tree_height = tree_height_by_length(self.length);

        // First, check an edge case when the list contains no elements.
        if tree_height == 0 {
            return if self.proof.is_empty() && self.entries.is_empty() {
                Ok(Hash::zero())
            } else {
                Err(ListProofError::NonEmptyProof)
            };
        }

        // Fast path in case there are no values: in this case, the proof can contain
        // only a single root hash.
        if self.entries.is_empty() {
            return match self.proof[..] {
                [] => Err(ListProofError::MissingHash),
                [HashedEntry { key, hash }] if key == ProofListKey::new(tree_height, 0) => Ok(hash),
                _ => Err(ListProofError::UnexpectedBranch),
            };
        }

        // Check ordering of `self.values` and `self.hashes`, which is relied upon
        // in the following steps.
        let values_ordered = self
            .entries
            .windows(2)
            .all(|window| window[0].0 < window[1].0);
        if !values_ordered {
            return Err(ListProofError::Unordered);
        }

        let hashes_ordered = self
            .proof
            .windows(2)
            .all(|window| window[0].key < window[1].key);
        if !hashes_ordered {
            return Err(ListProofError::Unordered);
        }

        // Check that hashes on each height have indexes in the allowed range.
        for &HashedEntry { key, .. } in &self.proof {
            let height = key.height();
            if height == 0 {
                return Err(ListProofError::UnexpectedLeaf);
            }

            // `self.length - 1` is the index of the last element at `height = 1`. This index
            // is divided by 2 with each new height.
            if height >= tree_height || key.index() > (self.length - 1) >> u64::from(height - 1) {
                return Err(ListProofError::UnexpectedBranch);
            }
        }

        let mut layer: Vec<_> = self
            .entries
            .iter()
            .map(|(i, value)| {
                HashedEntry::new(
                    ProofListKey::new(1, *i),
                    HashTag::hash_leaf(&value.to_bytes()),
                )
            })
            .collect();

        let mut hashes = self.proof.clone();
        // We track `last_index` instead of layer length in order to be able to more efficiently
        // update it when transitioning to the next height. It suffices to divide `last_index` by 2,
        // while if we used length, it would need to be modified as `l = (l + 1) / 2`.
        let mut last_index = self.length - 1;
        // We have covered `self.length == 0` case before, so the subtraction above is safe.

        for height in 1..tree_height {
            // We split `hashes` into those at `height` and those having greater height
            // (by construction, there may be no hashes with the lesser height).
            let split_key = ProofListKey::new(height + 1, 0);
            let split_index = hashes
                .binary_search_by(|entry| entry.key.cmp(&split_key))
                .unwrap_or_else(|i| i);
            let remaining_hashes = hashes.split_off(split_index);
            debug_assert!(
                hashes.iter().all(|entry| entry.key.height() == height),
                "Unexpected `hashes`: {:?}",
                hashes
            );
            debug_assert!(
                remaining_hashes
                    .first()
                    .map_or(true, |first| first.key.height() > height),
                "Unexpected `remaining_hashes`: {:?}",
                remaining_hashes
            );

            // Merge `hashes` with those obtained by zipping the previous layer.
            layer = merge(layer.into_iter(), hashes.into_iter())
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| ListProofError::RedundantHash)?;

            // Zip the current layer.
            hash_layer(&mut layer, last_index)?;
            last_index /= 2;
            hashes = remaining_hashes;
        }

        debug_assert_eq!(layer.len(), 1);
        debug_assert_eq!(layer[0].key, ProofListKey::new(tree_height, 0));
        Ok(layer[0].hash)
    }

    /// Returns the length of the underlying `ProofListIndex`.
    pub fn list_len(&self) -> u64 {
        self.length
    }

    /// Returns indexes and references to elements in the proof without verifying it.
    pub fn entries_unchecked(&self) -> &[(u64, V)] {
        &self.entries
    }

    /// Returns iterator over indexes of the elements in the proof without verifying
    /// proof integrity.
    pub fn indexes_unchecked<'s>(&'s self) -> impl Iterator<Item = u64> + 's {
        self.entries_unchecked().iter().map(|(index, _)| *index)
    }

    /// Provides access to the proof part of the view. Used in serialization.
    pub(crate) fn proof_unchecked(&self) -> &[HashedEntry] {
        &self.proof
    }

    /// Estimates the number of hash operations necessary to validate the proof.
    ///
    /// An error will be returned if the proof fails basic integrity checks. Not returning an error
    /// does not guarantee that the proof is valid, however; the estimation skips most
    /// of the checks for speed.
    pub fn hash_ops(&self) -> Result<usize, ListProofError> {
        // First, we need to hash all values in the proof.
        let mut hash_ops = self.entries.len();

        // Observe that the number of hashes known at each height of the Merkle tree
        // determines the number of hash operations necessary to produce hashes on the next height.
        // Thus, we just track the number of hashes known at each height.
        let mut hashes_on_this_height = hash_ops;
        let mut height = 1;

        for HashedEntry { key, .. } in &self.proof {
            // If `key.height()`s are not ordered, we know for sure that the proof is malformed.
            if key.height() < height {
                return Err(if height == 0 {
                    ListProofError::UnexpectedLeaf
                } else {
                    ListProofError::Unordered
                });
            }

            // Move hashes to the next height while possible. If `self.hashes` are sorted
            // (which they should be), we cannot get new hashes on the heights considered here
            // on the following `for` iterations.
            while key.height() > height {
                hashes_on_this_height = (hashes_on_this_height + 1) / 2;
                hash_ops += hashes_on_this_height;
                height += 1;
            }

            // If the proof is properly formed, hashes in `self.hashes` all have differing `key`s
            // among each other and with the hashes we can compute from earlier heights. Thus,
            // we can increment `hashes_on_this_height`.
            debug_assert_eq!(key.height(), height);
            hashes_on_this_height += 1;
        }

        // We've run out of hashes in the proof; now, we just successively zip hashes
        // until a single hash remains (this hash is the Merkle tree root).
        while hashes_on_this_height > 1 {
            hashes_on_this_height = (hashes_on_this_height + 1) / 2;
            hash_ops += hashes_on_this_height;
        }

        Ok(hash_ops)
    }

    /// Verifies the correctness of the proof.
    ///
    /// If the proof is valid, a checked list proof is returned, which allows to access
    /// proven elements.
    ///
    /// ## Errors
    ///
    /// An error is returned if proof is malformed. The following checks are performed:
    ///
    /// - `proof` field is ordered by increasing `(height, index)` tuple.
    /// - `entries` are ordered by increasing index.
    /// - Positions of elements in `proof` and `entries` are feasible.
    /// - There is sufficient information in `proof` and `entries` to restore the Merkle tree root.
    /// - There are no redundant entries in `proof` (i.e., ones that can be inferred from other
    ///   `proof` elements / `entries`).
    pub fn check(&self) -> Result<CheckedListProof<'_, V>, ListProofError> {
        let tree_root = self.collect()?;
        Ok(CheckedListProof {
            entries: &self.entries,
            length: self.length,
            hash: HashTag::hash_list_node(self.length, tree_root),
        })
    }

    /// Verifies the correctness of the proof according to the trusted list hash.
    ///
    /// The method is essentially a convenience wrapper around `check()`.
    ///
    /// # Return value
    ///
    /// If the proof is valid, a checked list proof is returned, which allows to access
    /// proven elements. Otherwise, an error is returned.
    pub fn check_against_hash(
        &self,
        expected_list_hash: Hash,
    ) -> Result<CheckedListProof<'_, V>, ValidationError<ListProofError>> {
        self.check()
            .map_err(ValidationError::Malformed)
            .and_then(|checked_proof| {
                if checked_proof.index_hash() == expected_list_hash {
                    Ok(checked_proof)
                } else {
                    Err(ValidationError::UnmatchedRootHash)
                }
            })
    }

    /// Creates `ListProof` from `proof` and `entries` vectors. Used to construct proof
    /// after deserialization.
    pub(crate) fn from_raw_parts(
        proof: Vec<HashedEntry>,
        entries: Vec<(u64, V)>,
        length: u64,
    ) -> Self {
        Self {
            proof,
            entries,
            length,
        }
    }
}

/// Version of `ListProof` obtained after verification.
///
/// See [`ListProof`] for an example of usage.
///
/// [`ListProof`]: struct.ListProof.html#workflow
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CheckedListProof<'a, V> {
    entries: &'a [(u64, V)],
    length: u64,
    hash: Hash,
}

impl<'a, V> CheckedListProof<'a, V> {
    /// Returns indexes and references to elements in the proof.
    pub fn entries(&self) -> &'a [(u64, V)] {
        self.entries
    }

    /// Returns iterator over indexes of the elements in the proof without verifying
    /// proof integrity.
    pub fn indexes<'s>(&'s self) -> impl Iterator<Item = u64> + 's {
        self.entries().iter().map(|(index, _)| *index)
    }

    /// Returns the length of the underlying `ProofListIndex`.
    pub fn list_len(&self) -> u64 {
        self.length
    }

    /// Returns the `object_hash()` of the underlying `ProofListIndex`.
    pub fn index_hash(&self) -> Hash {
        self.hash
    }
}

/// An error that is returned when the list proof is invalid.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Fail)]
pub enum ListProofError {
    /// Proof contains a hash in a place where a value was expected.
    #[fail(display = "proof contains a hash in a place where a value was expected")]
    UnexpectedLeaf,

    /// Proof contains a hash in the position which is impossible according to the list length.
    #[fail(
        display = "proof contains a hash in the position which is impossible according to the list length"
    )]
    UnexpectedBranch,

    /// Values or hashes in the proof are not ordered by their keys.
    #[fail(display = "values or hashes in the proof are not ordered by their keys")]
    Unordered,

    /// There are redundant hashes in the proof: the hash of the underlying list can be calculated
    /// without some of them.
    #[fail(display = "redundant hash in the proof")]
    RedundantHash,

    /// Proof does not contain necessary information to compute the hash of the underlying list.
    #[fail(display = "missing hash")]
    MissingHash,

    /// Non-empty proof for an empty list.
    ///
    /// Empty lists should always have empty proofs, since there is no data to get values
    /// or hashes from.
    #[fail(display = "non-empty proof for an empty list")]
    NonEmptyProof,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{access::AccessExt, Database, TemporaryDB};

    fn entry(height: u8, index: u64) -> HashedEntry {
        HashedEntry::new(
            ProofListKey::new(height, index),
            HashTag::hash_leaf(&index.to_bytes()),
        )
    }

    #[test]
    fn merge_example() {
        let first = vec![entry(1, 0), entry(1, 5), entry(2, 5)].into_iter();
        let second = vec![
            entry(1, 1),
            entry(2, 2),
            entry(2, 3),
            entry(3, 0),
            entry(4, 1),
        ]
        .into_iter();
        let merged = merge(first, second).collect::<Result<Vec<_>, _>>().unwrap();

        assert_eq!(
            merged,
            vec![
                entry(1, 0),
                entry(1, 1),
                entry(1, 5),
                entry(2, 2),
                entry(2, 3),
                entry(2, 5),
                entry(3, 0),
                entry(4, 1),
            ]
        );
    }

    #[test]
    fn hash_layer_example() {
        let mut layer = vec![
            entry(1, 0),
            entry(1, 1),
            entry(1, 6),
            entry(1, 7),
            entry(1, 8),
        ];
        hash_layer(&mut layer, 8).unwrap();
        assert!(layer.iter().map(|entry| entry.key).eq(vec![
            ProofListKey::new(2, 0),
            ProofListKey::new(2, 3),
            ProofListKey::new(2, 4),
        ]));

        assert_eq!(
            layer[0].hash,
            HashTag::hash_node(
                &HashTag::hash_leaf(&0_u64.to_bytes()),
                &HashTag::hash_leaf(&1_u64.to_bytes()),
            )
        );
        assert_eq!(
            layer[2].hash,
            HashTag::hash_single_node(&HashTag::hash_leaf(&8_u64.to_bytes()))
        );

        // layer[0] has odd index
        let mut layer = vec![entry(1, 1), entry(1, 2)];
        assert!(hash_layer(&mut layer, 2).is_err());

        // layer[1] is not adjacent to layer[0]
        let mut layer = vec![entry(1, 0), entry(1, 2)];
        assert!(hash_layer(&mut layer, 3).is_err());
        let mut layer = vec![entry(1, 0), entry(1, 3)];
        assert!(hash_layer(&mut layer, 3).is_err());

        // layer[-1] has odd index, while there is even number of elements in the layer
        let mut layer = vec![entry(1, 0), entry(1, 1), entry(1, 7)];
        assert!(hash_layer(&mut layer, 7).is_err());

        // layer[-1] has index lesser that the layer length
        let mut layer = vec![entry(1, 0), entry(1, 1), entry(1, 4)];
        assert!(hash_layer(&mut layer, 6).is_err());
    }

    #[test]
    fn hash_ops_examples() {
        // Empty proof.
        let proof = ListProof::<u32>::empty(Hash::zero(), 15);
        assert_eq!(proof.hash_ops().unwrap(), 0);

        // Proof for a single-element tree.
        let proof = ListProof::new(vec![(0, 0_u32)], 1);
        assert_eq!(proof.hash_ops().unwrap(), 1);

        // Proof for index 1 in a 3-element tree.
        let mut proof = ListProof::new(vec![(1, 1_u32)], 3);
        proof.push_hash(1, 0, Hash::zero());
        proof.push_hash(2, 1, Hash::zero());
        assert_eq!(proof.hash_ops().unwrap(), 3);
        // 1 ops to hash values + 1 ops on height 1 + 1 op on height 2:
        //
        //   root
        //  /    \
        //  *    x   Level #2
        // / \
        // x *       Level #1
        //   |
        //   x       Values

        // Proof for index 4 in a 5-element tree.
        let mut proof = ListProof::new(vec![(4, 4_u32)], 5);
        proof.push_hash(3, 0, Hash::zero());
        assert_eq!(proof.hash_ops().unwrap(), 4);
        // 1 ops to hash values + 1 op per heights 1..=3:
        //
        //   root
        //  /    \
        //  x    *   Level #3
        //       |
        //       *   Level #2
        //       |
        //       *   Level #1
        //       |
        //       x   Values

        // Proof for indexes 1..=2 in a 3-element tree.
        let mut proof = ListProof::new(vec![(1, 1_u32), (2, 2)], 3);
        proof.push_hash(1, 0, Hash::zero());
        assert_eq!(proof.hash_ops().unwrap(), 5);
        // 2 ops to hash values + 2 ops on height 1 + 1 op on height 2:
        //
        //   root
        //  /    \
        //  *    *   Level #2
        // / \   |
        // x *   *   Level #1
        //   |   |
        //   x   x   Values
    }

    #[test]
    fn hash_ops_in_full_tree() {
        // Consider a graph for computing Merkle root of the tree, such as one depicted above.
        // Denote `l` the number of leaves in this tree (i.e., nodes with degree 1),
        // `v2` number of nodes with degree 2, and `v3` the number of nodes with degree 3.
        // For example, in the tree above, `l = 3`, `v2 = 3`, `v3 = 1`.
        //
        // We have
        //
        //     l = proof.values.len() + proof.hashes.len().
        //
        // The number of edges in the tree is `|E| = (l + 2*v2 + 3*v3) / 2`; on the other hand,
        // it is connected to the number of nodes `|E| = l + v2 + v3 - 1`. Hence,
        //
        //     v3 = l - 2.
        //
        // The number of hash operations is
        //
        //     Ops = v2 + v3 = l + v2 - 2.
        //
        // `v2` counts at least values and the root node (provided there is >1 node in the tree);
        // i.e.,
        //
        //     v2  >= proof.values.len() + 1;
        //     Ops >= 2 * proof.values.len() + proof.hashes.len() - 1.
        //
        // For a full Merkle tree, this becomes an equality, as there can be no other degree-2
        // nodes in the proof.

        let db = TemporaryDB::new();
        let fork = db.fork();
        let mut list = fork.get_proof_list("test");
        list.extend(0_u32..8);

        for len in 1..8 {
            for i in 0..(8 - len) {
                let proof = list.get_range_proof(i..(i + len));
                assert_eq!(
                    proof.hash_ops().unwrap(),
                    2 * proof.entries.len() + proof.proof.len() - 1,
                    "{:?}",
                    proof
                );
            }
        }
    }
}
