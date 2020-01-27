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

//! Building `MapProof`s. See README.md in the module directory for high-level explanation
//! how the proofs are built.

use std::borrow::Borrow;

use exonum_crypto::Hash;

use super::{
    key::{BitsRange, ChildKind, ProofPath},
    node::{BranchNode, Node},
    MapProof, ToProofPath,
};
use crate::BinaryKey;

// Expected size of the proof, in number of hashed entries.
const DEFAULT_PROOF_CAPACITY: usize = 8;

impl<K, V, KeyMode> MapProof<K, V, KeyMode> {
    /// Includes a proof of existence / absence of a single key when a proof of multiple
    /// keys is requested.
    fn process_key<Q: ?Sized>(
        mut self,
        tree: &impl MerklePatriciaTree<Q, V>,
        contour: &mut Vec<ContourNode>,
        proof_path: &ProofPath,
        key: K,
    ) -> Self
    where
        K: Borrow<Q>,
    {
        // `unwrap()` is safe: there is at least 1 element in the contour by design
        let common_prefix = proof_path.common_prefix_len(&contour.last().unwrap().key);

        // Eject nodes from the contour while they will they can be "finalized"
        while let Some(node) = contour.pop() {
            if contour.is_empty() || node.key.len() <= common_prefix {
                contour.push(node);
                break;
            } else {
                self = node.add_to_proof(self);
            }
        }

        // Push new items to the contour.
        loop {
            let contour_tip = contour.last_mut().unwrap();
            let next_height = contour_tip.key.len();
            let next_bit = proof_path.bit(next_height);
            let node_path = contour_tip.branch.child_path(next_bit);

            if proof_path.matches_from(&node_path, next_height) {
                match next_bit {
                    ChildKind::Left => contour_tip.visited_left = true,
                    ChildKind::Right => {
                        if !contour_tip.visited_left {
                            self = self.add_proof_entry(
                                contour_tip.branch.child_path(ChildKind::Left),
                                contour_tip.branch.child_hash(ChildKind::Left),
                            );
                        }
                        contour_tip.visited_right = true;
                    }
                }
            } else {
                // Both children of `branch` do not fit; stop here
                break self.add_missing(key);
            }

            let node = tree.node(&node_path);
            match node {
                Node::Branch(branch) => {
                    contour.push(ContourNode::new(node_path, branch));
                }
                Node::Leaf(_) => {
                    // We have reached the leaf node and haven't diverged!
                    let value = tree.value(key.borrow());
                    break self.add_entry(key, value);
                }
            }
        }
    }
}

/// Encapsulation of a Merkle Patricia tree allowing to access its terminal and intermediate
/// nodes.
pub trait MerklePatriciaTree<K: ?Sized, V> {
    /// Gets the root node of the tree.
    fn root_node(&self) -> Option<(ProofPath, Node)>;

    /// Gets the node by its `path`.
    ///
    /// It is assumed that this method cannot fail since it is queried with `path`s
    /// that are guaranteed to be present in the tree.
    fn node(&self, path: &ProofPath) -> Node;

    /// Looks up the value by its full key.
    ///
    /// It is assumed that this method cannot fail since it is queried with `key`s
    /// that are guaranteed to be present in the tree.
    fn value(&self, key: &K) -> V;
}

/// Combines two lists of hashes produces when building a `MapProof`.
///
/// # Invariants
///
/// - `left_hashes` need to be ordered by increasing `ProofPath`.
/// - `right_hashes` need to be ordered by decreasing `ProofPath`.
fn combine_hashes(
    mut left_hashes: Vec<(ProofPath, Hash)>,
    right_hashes: Vec<(ProofPath, Hash)>,
) -> Vec<(ProofPath, Hash)> {
    left_hashes.extend(right_hashes.into_iter().rev());
    left_hashes
}

/// Nodes in the contour during creation of multi-proofs.
#[derive(Debug)]
struct ContourNode {
    key: ProofPath,
    branch: BranchNode,
    visited_left: bool,
    visited_right: bool,
}

impl ContourNode {
    fn new(key: ProofPath, branch: BranchNode) -> Self {
        Self {
            key,
            branch,
            visited_left: false,
            visited_right: false,
        }
    }

    // Adds this contour node into a proof builder.
    fn add_to_proof<K, V, KeyMode>(
        self,
        mut builder: MapProof<K, V, KeyMode>,
    ) -> MapProof<K, V, KeyMode> {
        if !self.visited_right {
            // This works due to the following observation: If neither of the child nodes
            // were visited when the node is being ejected from the contour,
            // this means that it is safe to add the left and right hashes (in this order)
            // to the proof. The observation is provable by induction.
            if !self.visited_left {
                builder = builder.add_proof_entry(
                    self.branch.child_path(ChildKind::Left),
                    self.branch.child_hash(ChildKind::Left),
                );
            }

            builder = builder.add_proof_entry(
                self.branch.child_path(ChildKind::Right),
                self.branch.child_hash(ChildKind::Right),
            );
        }
        builder
    }
}

/// Builds proofs for arbitrary set of keys in a Merkelized map.
///
/// This is an extension trait to [`MerklePatriciaTree`]; all types implementing
/// `MerklePatriciaTree` (with reasonable constraints on key and value types) automatically
/// implement `BuildProof` as well.
///
/// [`MerklePatriciaTree`]: trait.MerklePatriciaTree.html
pub trait BuildProof<K: ToOwned + ?Sized, V, KeyMode> {
    /// Creates a proof of existence / absence for a single key.
    fn create_proof(&self, key: K::Owned) -> MapProof<K::Owned, V, KeyMode>;

    /// Creates a proof of existence / absence for multiple keys.
    fn create_multiproof(
        &self,
        keys: impl IntoIterator<Item = K::Owned>,
    ) -> MapProof<K::Owned, V, KeyMode>;
}

impl<K, V, T, KeyMode> BuildProof<K, V, KeyMode> for T
where
    K: BinaryKey + ?Sized,
    T: MerklePatriciaTree<K, V>,
    KeyMode: ToProofPath<K>,
{
    fn create_proof(&self, key: K::Owned) -> MapProof<K::Owned, V, KeyMode> {
        let searched_path = KeyMode::transform_key(key.borrow());
        match self.root_node() {
            Some((root_path, Node::Branch(root_branch))) => {
                let mut left_hashes = Vec::with_capacity(DEFAULT_PROOF_CAPACITY);
                let mut right_hashes = Vec::with_capacity(DEFAULT_PROOF_CAPACITY);

                // Currently visited branch and its key, respectively.
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
                        (branch.child_path(!next_bit), branch.child_hash(!next_bit));
                    match !next_bit {
                        ChildKind::Left => left_hashes.push(other_path_and_hash),
                        ChildKind::Right => right_hashes.push(other_path_and_hash),
                    }

                    if searched_path.matches_from(&node_path, next_height) {
                        match self.node(&node_path) {
                            Node::Branch(child_branch) => branch = child_branch,
                            Node::Leaf(_) => {
                                // We have reached the leaf node and haven't diverged!
                                // The key is there, we've just gotten the value, so we just
                                // need to return it.
                                let value = self.value(key.borrow());
                                break MapProof::new()
                                    .add_entry(key, value)
                                    .add_proof_entries(combine_hashes(left_hashes, right_hashes));
                            }
                        }
                    } else {
                        // Both children of `branch` do not fit.
                        let next_hash = branch.child_hash(next_bit);
                        match next_bit {
                            ChildKind::Left => left_hashes.push((node_path, next_hash)),
                            ChildKind::Right => right_hashes.push((node_path, next_hash)),
                        }

                        break MapProof::new()
                            .add_missing(key)
                            .add_proof_entries(combine_hashes(left_hashes, right_hashes));
                    }
                }
            }

            Some((root_path, Node::Leaf(hash))) => {
                if root_path == searched_path {
                    let value = self.value(key.borrow());
                    MapProof::new().add_entry(key, value)
                } else {
                    MapProof::new()
                        .add_missing(key)
                        .add_proof_entry(root_path, hash)
                }
            }

            None => MapProof::new().add_missing(key),
        }
    }

    fn create_multiproof(
        &self,
        keys: impl IntoIterator<Item = K::Owned>,
    ) -> MapProof<K::Owned, V, KeyMode> {
        match self.root_node() {
            Some((root_path, Node::Branch(root_branch))) => {
                let mut proof: MapProof<K::Owned, V, KeyMode> = MapProof::new();

                let searched_paths = {
                    let mut keys: Vec<_> = keys
                        .into_iter()
                        .map(|k| (KeyMode::transform_key(k.borrow()), k))
                        .collect();

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
                    proof = proof.process_key(self, &mut contour, &proof_path, key);
                    last_searched_path = Some(proof_path);
                }

                // Eject remaining entries from the contour
                while let Some(node) = contour.pop() {
                    proof = node.add_to_proof(proof);
                }
                proof
            }
            Some((root_path, Node::Leaf(merkle_root))) => {
                let mut proof = MapProof::new();
                // (One of) keys corresponding to the existing table entry.
                let mut found_key: Option<K::Owned> = None;

                for key in keys {
                    let searched_path = KeyMode::transform_key(key.borrow());
                    if root_path == searched_path {
                        found_key = Some(key);
                    } else {
                        proof = proof.add_missing(key);
                    }
                }

                if let Some(key) = found_key {
                    let value = self.value(key.borrow());
                    proof.add_entry(key, value)
                } else {
                    proof.add_proof_entry(root_path, merkle_root)
                }
            }

            None => keys
                .into_iter()
                .fold(MapProof::new(), MapProof::add_missing),
        }
    }
}
