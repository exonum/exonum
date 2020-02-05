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

//! Building `ListProof`s.

use exonum_crypto::Hash;

use std::ops::{Bound, RangeBounds};

use super::{key::ProofListKey, tree_height_by_length, ListProof};
use crate::BinaryValue;

/// Encapsulation of a binary Merkle tree allowing to access its terminal and intermediate
/// nodes.
pub trait MerkleTree<V> {
    /// Gets the length of the tree.
    fn len(&self) -> u64;

    /// Gets the node by its `position` in the tree.
    ///
    /// It is assumed that this method cannot fail since it is queried with `position`s
    /// that are guaranteed to be present in the tree.
    fn node(&self, position: ProofListKey) -> Hash;

    /// Iterates over values starting from the specified index.
    fn values<'s>(&'s self, start_index: u64) -> Box<dyn Iterator<Item = V> + 's>;

    /// Gets the Merkle root of the tree.
    fn merkle_root(&self) -> Hash {
        let tree_height = tree_height_by_length(self.len());
        self.node(ProofListKey::new(tree_height, 0))
    }
}

pub trait BuildProof<V> {
    fn create_proof(&self, index: u64) -> ListProof<V>;
    fn create_range_proof(&self, indexes: impl RangeBounds<u64>) -> ListProof<V>;
}

impl<V, T> BuildProof<V> for T
where
    V: BinaryValue,
    T: MerkleTree<V>,
{
    fn create_proof(&self, index: u64) -> ListProof<V> {
        create_proof(self, index, index)
    }

    fn create_range_proof(&self, indexes: impl RangeBounds<u64>) -> ListProof<V> {
        // Inclusive lower boundary of the proof range.
        let from = match indexes.start_bound() {
            Bound::Unbounded => 0_u64,
            Bound::Included(from) => *from,
            Bound::Excluded(from) => *from + 1,
        };

        // Exclusive upper boundary of the proof range.
        let to = match indexes.end_bound() {
            Bound::Unbounded => self.len(),
            // Saturation below doesn't matter: if `to == u64::max_value()`, it is guaranteed
            // to be larger than any possible list length.
            Bound::Included(to) => to.saturating_add(1),
            Bound::Excluded(to) => *to,
        };

        if (from >= self.len() && indexes.end_bound() == Bound::Unbounded) || from == to {
            // We assume the first condition is a "legal" case of the caller not knowing
            // the list length, so we don't want to panic in the `to > from` assertion below.
            return ListProof::empty(self.merkle_root(), self.len());
        }
        assert!(
            to > from,
            "Illegal range boundaries: the range start is {}, but the range end is {}",
            from,
            to
        );
        create_proof(self, from, to - 1)
    }
}

/// Creates a `ListProof` for a contiguous closed range of indexes `[from, inclusive_to]`.
///
/// The caller must ensure that `inclusive_to >= from`.
fn create_proof<V: BinaryValue>(
    tree: &impl MerkleTree<V>,
    from: u64,
    inclusive_to: u64,
) -> ListProof<V> {
    let tree_len = tree.len();
    let tree_height = tree_height_by_length(tree_len);
    if from >= tree_len {
        return ListProof::empty(tree.merkle_root(), tree_len);
    }

    let items = (from..=inclusive_to).zip(tree.values(from));
    let mut proof = ListProof::new(items, tree_len);

    // `left` and `right` track the indexes of elements for which we build the proof,
    // on the particular `height` of the tree. Both these values are inclusive; i.e., the range
    // is `[left, right]`.
    let (mut left, mut right) = (from, inclusive_to);
    let mut last_index_on_level = tree_len - 1;

    for height in 1..tree_height {
        // On each `height`, we may include into the proof the hash
        // to the left of the range, provided that this hash is necessary to restore
        // the root hash of the list. It is easy to see that necessity depends on the
        // oddity of `left`. Indeed, during root hash computation, we zip a hash
        // with an *even* index with the following hash having an *odd* index;
        // thus, iff `left` is odd, we need a hash with index `left - 1` to restore the
        // root hash.
        if left % 2 == 1 {
            let hash = tree.node(ProofListKey::new(height, left - 1));
            proof.push_hash(height, left - 1, hash);
        }

        // Similarly, we may need a hash to the right of the end of the range, provided
        // that the end has an even index and the hash to the right exists.
        if right % 2 == 0 && right < last_index_on_level {
            let hash = tree.node(ProofListKey::new(height, right + 1));
            proof.push_hash(height, right + 1, hash);
        }

        left /= 2;
        right /= 2;
        last_index_on_level /= 2;
    }
    proof
}
