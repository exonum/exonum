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

#![allow(unsafe_code)]

use std::borrow::Cow;

use failure::{self, ensure};
use smallvec::{smallvec, SmallVec};

use exonum_crypto::{self, Hash, HASH_SIZE};

use super::key::{ChildKind, ProofPath, PROOF_PATH_SIZE};
use crate::{BinaryKey, BinaryValue, HashTag, ObjectHash};

const BRANCH_NODE_SIZE: usize = 2 * (HASH_SIZE + PROOF_PATH_SIZE);

/// Enum that represents the type of the `ProofMap` node.
#[derive(Debug)]
pub enum Node {
    /// Leaf node that contains a hash of the leaf value.
    Leaf(Hash),
    /// Branch node.
    Branch(BranchNode),
}

/// Structure that encapsulates a raw representation of the branch node.
#[derive(Clone, PartialEq)]
pub struct BranchNode {
    raw: Vec<u8>,
}

impl BranchNode {
    /// Create empty `BranchNode`.
    pub fn empty() -> Self {
        Self {
            raw: vec![0_u8; BRANCH_NODE_SIZE],
        }
    }

    pub(crate) fn child_hash(&self, kind: ChildKind) -> Hash {
        let from = match kind {
            ChildKind::Right => HASH_SIZE,
            ChildKind::Left => 0,
        };
        Hash::read(&self.raw[from..from + HASH_SIZE])
    }

    pub(crate) fn child_path(&self, kind: ChildKind) -> ProofPath {
        let from = match kind {
            ChildKind::Right => 2 * HASH_SIZE + PROOF_PATH_SIZE,
            ChildKind::Left => 2 * HASH_SIZE,
        };
        ProofPath::read(&self.raw[from..from + PROOF_PATH_SIZE])
    }

    pub(crate) fn set_child_path(&mut self, kind: ChildKind, prefix: &ProofPath) {
        let from = match kind {
            ChildKind::Right => 2 * HASH_SIZE + PROOF_PATH_SIZE,
            ChildKind::Left => 2 * HASH_SIZE,
        };
        prefix.write(&mut self.raw[from..from + PROOF_PATH_SIZE]);
    }

    pub(crate) fn set_child_hash(&mut self, kind: ChildKind, hash: &Hash) {
        let from = match kind {
            ChildKind::Right => HASH_SIZE,
            ChildKind::Left => 0,
        };
        self.raw[from..from + HASH_SIZE].copy_from_slice(hash.as_ref());
    }

    pub(crate) fn set_child(&mut self, kind: ChildKind, prefix: &ProofPath, hash: &Hash) {
        self.set_child_path(kind, prefix);
        self.set_child_hash(kind, hash);
    }
}

impl BinaryValue for BranchNode {
    fn to_bytes(&self) -> Vec<u8> {
        self.raw.clone()
    }

    fn into_bytes(self) -> Vec<u8> {
        self.raw
    }

    fn from_bytes(bytes: Cow<'_, [u8]>) -> Result<Self, failure::Error> {
        let raw = bytes.into_owned();
        ensure!(raw.len() == BRANCH_NODE_SIZE, "Wrong buffer size");
        Ok(Self { raw })
    }
}

impl ObjectHash for BranchNode {
    fn object_hash(&self) -> Hash {
        let mut bytes: SmallVec<[u8; 256]> = smallvec![0_u8; 132];
        let mut pos = HASH_SIZE * 2;
        // Writes hashes to the buffer.
        bytes[..pos].copy_from_slice(&self.raw[..pos]);
        pos += self
            .child_path(ChildKind::Left)
            .write_compressed(&mut bytes[pos..]);
        pos += self
            .child_path(ChildKind::Right)
            .write_compressed(&mut bytes[pos..]);
        HashTag::hash_map_branch(&bytes[..pos])
    }
}

impl std::fmt::Debug for BranchNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BranchNode")
            .field("left_path", &self.child_path(ChildKind::Left))
            .field("left_hash", &self.child_hash(ChildKind::Left))
            .field("right_path", &self.child_path(ChildKind::Right))
            .field("right_hash", &self.child_hash(ChildKind::Right))
            .field("hash", &self.object_hash())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use exonum_crypto;

    use super::*;
    use crate::{proof_map::key::BitsRange, BinaryValue, ObjectHash};

    #[test]
    fn test_branch_node_layout() {
        let mut branch = BranchNode::empty();

        let lh = exonum_crypto::hash(&[1, 2]);
        let rh = exonum_crypto::hash(&[3, 4]);
        let ls = ProofPath::from_bytes(&[253; 32]);
        let rs = ProofPath::from_bytes(&[244; 32]);

        branch.set_child(ChildKind::Left, &ls, &lh);
        branch.set_child(ChildKind::Right, &rs, &rh);

        assert_eq!(branch.child_hash(ChildKind::Left), lh);
        assert_eq!(branch.child_hash(ChildKind::Right), rh);
        assert_eq!(branch.child_path(ChildKind::Left), ls);
        assert_eq!(branch.child_path(ChildKind::Right), rs);
    }

    #[test]
    fn test_branch_node_storage_value() {
        let mut branch = BranchNode::empty();

        let lh = exonum_crypto::hash(&[1, 2]);
        let rh = exonum_crypto::hash(&[3, 4]);
        let ls = ProofPath::from_bytes(&[253; 32]).suffix(9).prefix(15);
        let rs = ProofPath::from_bytes(&[244; 32]);

        branch.set_child(ChildKind::Left, &ls, &lh);
        branch.set_child(ChildKind::Right, &rs, &rh);

        let buf = branch.to_bytes();
        let branch2 = BranchNode::from_bytes(buf.into()).unwrap();
        assert_eq!(branch, branch2);
        assert_eq!(branch.object_hash(), branch2.object_hash());
        assert_eq!(
            branch.object_hash().to_hex(),
            "2a179b2c4d93c88128a37ad9aae0e749bf9848f9c2a2463ebb6b53b31792a8bb"
        );
    }
}
