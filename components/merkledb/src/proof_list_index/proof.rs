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

use exonum_crypto::Hash;
use serde_derive::*;

use std::cmp::Ordering;

use super::{key::ProofListKey, tree_height_by_length};
use crate::{BinaryValue, HashTag};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
struct HashedEntry {
    #[serde(flatten)]
    key: ProofListKey,
    hash: Hash,
}

impl HashedEntry {
    fn new(key: ProofListKey, hash: Hash) -> Self {
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
/// [`validate()`]. Prior to the `validate` conversion, you may use `*unchecked` methods
/// to obtain information about the proof.
///
/// ```
/// # use exonum_merkledb::{
/// #     Database, TemporaryDB, BinaryValue, ListProof, ProofListIndex, ObjectHash,
/// # };
/// # use exonum_crypto::hash;
/// let fork = { let db = TemporaryDB::new(); db.fork() };
/// let mut list = ProofListIndex::new("index", &fork);
/// list.extend(vec![100_u32, 200_u32, 300_u32]);
///
/// // Get the proof from the index
/// let proof = list.get_range_proof(1..);
///
/// // Check the proof consistency
/// let elements = proof.validate(list.object_hash(), list.len()).unwrap();
/// assert_eq!(*elements, [(1, 200_u32), (2, 300_u32)]);
/// ```
///
/// # JSON serialization
///
/// `ListProof` is serialized to JSON as an object with 2 array fields:
///
/// - `hashes` is an array of `{ height: number, index: number, hash: Hash }` objects.
/// - `values` is an array with list elements and their indexes, that is,
///   tuples `[number, V]`.
///
/// ```
/// # use serde_json::{self, json};
/// # use exonum_merkledb::{Database, TemporaryDB, BinaryValue, HashTag, ListProof, ProofListIndex};
/// # fn main() {
/// let fork = { let db = TemporaryDB::new(); db.fork() };
/// let mut list = ProofListIndex::new("index", &fork);
/// list.extend(vec![1_u32, 2, 3]);
/// let h1 = HashTag::hash_leaf(&1_u32.to_bytes());
/// let h3 = HashTag::hash_leaf(&3_u32.to_bytes());
/// let h33 = HashTag::hash_single_node(&h3);
///
/// let proof = list.get_proof(1);
/// assert_eq!(
///     serde_json::to_value(&proof).unwrap(),
///     json!({
///         "hashes": [
///             { "index": 0, "height": 1, "hash": h1 },
///             { "index": 1, "height": 2, "hash": h33 },
///         ],
///         "values": [ [1, 2] ],
///     })
/// );
/// # }
/// ```
///
/// [`get_proof()`]: struct.ProofListIndex.html#method.get_proof
/// [`get_range_proof()`]: struct.ProofListIndex.html#method.get_range_proof
/// [`validate()`]: #method.validate
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct ListProof<V> {
    hashes: Vec<HashedEntry>,
    values: Vec<(u64, V)>,
}

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

fn hash_layer(layer: &[HashedEntry], last_index: u64) -> Result<Vec<HashedEntry>, ListProofError> {
    let mut hashed = Vec::with_capacity(layer.len() / 2 + 1);

    for chunk in layer.chunks(2) {
        match *chunk {
            [x, y] => {
                if !x.key.is_left() || y.key.index() != x.key.index() + 1 {
                    return Err(ListProofError::MissingHash);
                }

                hashed.push(HashedEntry::new(
                    x.key.parent(),
                    HashTag::hash_node(&x.hash, &y.hash),
                ));
            }

            [last] => {
                if last_index % 2 == 1 || last.key.index() != last_index {
                    return Err(ListProofError::MissingHash);
                }

                hashed.push(HashedEntry::new(
                    last.key.parent(),
                    HashTag::hash_single_node(&last.hash),
                ));
            }

            _ => unreachable!(),
        }
    }

    Ok(hashed)
}

impl<V: BinaryValue> ListProof<V> {
    pub(super) fn new<I>(values: I) -> Self
    where
        I: IntoIterator<Item = (u64, V)>,
    {
        Self {
            values: values.into_iter().collect(),
            hashes: vec![],
        }
    }

    pub(super) fn empty(height: u8, merkle_root: Hash) -> Self {
        Self {
            values: vec![],
            hashes: vec![HashedEntry {
                key: ProofListKey::new(height, 0),
                hash: merkle_root,
            }],
        }
    }

    pub(super) fn push_hash(&mut self, height: u8, index: u64, hash: Hash) -> &mut Self {
        debug_assert!(height > 0);

        let key = ProofListKey::new(height, index);
        debug_assert!(
            if let Some(&HashedEntry { key: last_key, .. }) = self.hashes.last() {
                key > last_key
            } else {
                true
            }
        );

        self.hashes.push(HashedEntry::new(key, hash));
        self
    }

    fn collect(&self, list_len: u64) -> Result<Hash, ListProofError> {
        let tree_height = tree_height_by_length(list_len);
        if tree_height == 0 && (!self.hashes.is_empty() || !self.values.is_empty()) {
            return Err(ListProofError::NonEmptyProof);
        }
        // Fast path in case there are no values: in this case, the proof can contain
        // only a single root hash.
        if self.values.is_empty() {
            return match self.hashes[..] {
                [] => Err(ListProofError::MissingHash),
                [HashedEntry { key, hash }] if key == ProofListKey::new(tree_height, 0) => Ok(hash),
                _ => Err(ListProofError::UnexpectedBranch),
            };
        }

        let values_ordered = self
            .values
            .windows(2)
            .all(|window| window[0].0 < window[1].0);
        if !values_ordered {
            return Err(ListProofError::Unordered);
        }

        let hashes_ordered = self
            .hashes
            .windows(2)
            .all(|window| window[0].key < window[1].key);
        if !hashes_ordered {
            return Err(ListProofError::Unordered);
        }

        for &HashedEntry { key, .. } in &self.hashes {
            let height = key.height();

            if height == 0 {
                return Err(ListProofError::UnexpectedLeaf);
            }
            if height >= tree_height || key.index() >= 1 << u64::from(tree_height - height) {
                return Err(ListProofError::UnexpectedBranch);
            }
        }

        let mut layer: Vec<_> = self
            .values
            .iter()
            .map(|(i, value)| {
                HashedEntry::new(
                    ProofListKey::new(1, *i),
                    HashTag::hash_leaf(&value.to_bytes()),
                )
            })
            .collect();

        let mut hashes = self.hashes.clone();
        // We have covered `list_len == 0` case before, so this subtraction is safe.
        let mut last_index = list_len - 1;

        for height in 1..tree_height {
            let split_index = hashes.iter().position(|entry| entry.key.height() > height);
            let remaining_hashes = if let Some(i) = split_index {
                hashes.split_off(i)
            } else {
                vec![]
            };

            let merged = merge(layer.into_iter(), hashes.into_iter())
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| ListProofError::RedundantHash)?;

            layer = hash_layer(&merged, last_index)?;
            last_index >>= 1;
            hashes = remaining_hashes;
        }

        debug_assert_eq!(layer.len(), 1);
        debug_assert_eq!(layer[0].key, ProofListKey::new(tree_height, 0));
        Ok(layer[0].hash)
    }

    /// Returns indices and references to elements in the proof without verifying it.
    pub fn values_unchecked(&self) -> &[(u64, V)] {
        &self.values
    }

    /// Returns iterator over indexes of the elements in the proof without verifying
    /// proof integrity.
    pub fn indexes_unchecked<'s>(&'s self) -> impl Iterator<Item = u64> + 's {
        self.values_unchecked().iter().map(|(index, _)| *index)
    }

    /// Verifies the correctness of the proof by the trusted Merkle root hash and the number of
    /// elements in the tree.
    ///
    /// If the proof is valid, a vector with indices and references to elements is returned.
    /// Otherwise, an error is returned.
    pub fn validate(
        &self,
        expected_list_hash: Hash,
        len: u64,
    ) -> Result<&[(u64, V)], ListProofError> {
        let tree_root = self.collect(len)?;
        if HashTag::hash_list_node(len, tree_root) == expected_list_hash {
            Ok(&self.values)
        } else {
            Err(ListProofError::UnmatchedRootHash)
        }
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

    /// The hash of the proof is not equal to the trusted root hash.
    #[fail(display = "hash of the proof is not equal to the trusted hash of the list")]
    UnmatchedRootHash,

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
        let layer = vec![
            entry(1, 0),
            entry(1, 1),
            entry(1, 6),
            entry(1, 7),
            entry(1, 8),
        ];
        let hashed = hash_layer(&layer, 8).unwrap();
        assert!(hashed.iter().map(|entry| entry.key).eq(vec![
            ProofListKey::new(2, 0),
            ProofListKey::new(2, 3),
            ProofListKey::new(2, 4),
        ]));

        assert_eq!(
            hashed[0].hash,
            HashTag::hash_node(
                &HashTag::hash_leaf(&0_u64.to_bytes()),
                &HashTag::hash_leaf(&1_u64.to_bytes()),
            )
        );
        assert_eq!(
            hashed[2].hash,
            HashTag::hash_single_node(&HashTag::hash_leaf(&8_u64.to_bytes()))
        );

        // layer[0] has odd index
        let layer = vec![entry(1, 1), entry(1, 2)];
        assert!(hash_layer(&layer, 2).is_err());

        // layer[1] is not adjacent to layer[0]
        let layer = vec![entry(1, 0), entry(1, 2)];
        assert!(hash_layer(&layer, 3).is_err());
        let layer = vec![entry(1, 0), entry(1, 3)];
        assert!(hash_layer(&layer, 3).is_err());

        // layer[-1] has odd index, while there is even number of elements in the layer
        let layer = vec![entry(1, 0), entry(1, 1), entry(1, 7)];
        assert!(hash_layer(&layer, 7).is_err());

        // layer[-1] has index lesser that the layer length
        let layer = vec![entry(1, 0), entry(1, 1), entry(1, 4)];
        assert!(hash_layer(&layer, 6).is_err());
    }
}
