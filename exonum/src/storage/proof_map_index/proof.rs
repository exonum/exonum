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

use std::fmt;

use serde::{Serialize, Serializer};
use serde::ser::SerializeMap;

use crypto::{Hash, HashStream};

use super::super::{StorageValue, Error};
use super::key::{ProofMapKey, DBKey, ChildKind, KEY_SIZE};

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

/// An enum that represents a proof of existence or non-existence for a proof map key.
pub enum MapProof<V> {
    /// A boundary case with a single element tree and a matching key.
    LeafRootInclusive(DBKey, V),
    /// A boundary case with a single element tree and a non-matching key
    LeafRootExclusive(DBKey, Hash),
    /// A boundary case with empty tree.
    Empty,
    /// A root branch of the tree.
    Branch(BranchProofNode<V>),
}

/// An enum that represents a node of the map proof.
pub enum ProofNode<V> {
    /// A branch of map proof.
    Branch(BranchProofNode<V>),
    /// A leaf of map proof with the value of the requested key.
    Leaf(V),
}

/// An enum that represents a branch node of the map proof.
pub enum BranchProofNode<V> {
    /// A branch of proof in which both children do not contain the requested key.
    BranchKeyNotFound {
        /// A hash of the left child.
        left_hash: Hash,
        /// A hash of the right child.
        right_hash: Hash,
        /// A key of the left child.
        left_key: DBKey,
        /// A key of the right child.
        right_key: DBKey,
    },
    /// A branch of proof in which left child may contain the requested key.
    LeftBranch {
        /// A left child node.
        left_node: Box<ProofNode<V>>,
        /// A hash of the right child.
        right_hash: Hash,
        /// A key of the left child.
        left_key: DBKey,
        /// A key of the right child.
        right_key: DBKey,
    },
    /// A branch of proof in which right child may contain the requested key.
    RightBranch {
        /// A hash of the left child.
        left_hash: Hash,
        /// A right child node.
        right_node: Box<ProofNode<V>>,
        /// A key of the left child.
        left_key: DBKey,
        /// A key of the right child.
        right_key: DBKey,
    },
}

impl<V: StorageValue> MapProof<V> {
    /// Returns root hash of the map proof.
    pub fn root_hash(&self) -> Hash {
        use self::MapProof::*;
        match *self {
            Empty => Hash::zero(),
            LeafRootInclusive(ref root_key, ref root_val) => {
                HashStream::new()
                    .update(&root_key.as_bytes())
                    .update(root_val.hash().as_ref())
                    .hash()
            }
            LeafRootExclusive(ref root_key, ref root_val_hash) => {
                HashStream::new()
                    .update(&root_key.as_bytes())
                    .update(root_val_hash.as_ref())
                    .hash()
            }
            Branch(ref branch) => branch.root_hash(),
        }
    }
}
impl<V: StorageValue> ProofNode<V> {
    fn root_hash(&self) -> Hash {
        use self::ProofNode::*;
        match *self {
            Leaf(ref val) => val.hash(),
            Branch(ref branch) => branch.root_hash(),
        }
    }
}

impl<V: StorageValue> BranchProofNode<V> {
    fn root_hash(&self) -> Hash {
        use self::BranchProofNode::*;
        match *self {
            BranchKeyNotFound {
                ref left_hash,
                ref right_hash,
                ref left_key,
                ref right_key,
            } => {
                HashStream::new()
                    .update(left_hash.as_ref())
                    .update(right_hash.as_ref())
                    .update(&left_key.as_bytes())
                    .update(&right_key.as_bytes())
                    .hash()
            }
            LeftBranch {
                ref left_node,
                ref right_hash,
                ref left_key,
                ref right_key,
            } => {
                HashStream::new()
                    .update(left_node.root_hash().as_ref())
                    .update(right_hash.as_ref())
                    .update(&left_key.as_bytes())
                    .update(&right_key.as_bytes())
                    .hash()
            }
            RightBranch {
                ref left_hash,
                ref right_node,
                ref left_key,
                ref right_key,
            } => {
                HashStream::new()
                    .update(left_hash.as_ref())
                    .update(right_node.root_hash().as_ref())
                    .update(&left_key.as_bytes())
                    .update(&right_key.as_bytes())
                    .hash()
            }
        }
    }
}


impl<V: Serialize> Serialize for MapProof<V> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use self::MapProof::*;

        match *self {
            Empty => {
                let state = serializer.serialize_map(Some(0))?;
                state.end()
            }
            LeafRootInclusive(ref key, ref value) => {
                #[derive(Serialize)]
                struct SerializeHelper<'a, V: Serialize + 'a> {
                    val: &'a V,
                }
                let helper = SerializeHelper { val: value };
                let mut state = serializer.serialize_map(Some(1))?;
                state.serialize_entry(key, &helper)?;
                state.end()
            }
            LeafRootExclusive(ref key, ref hash) => {
                let mut state = serializer.serialize_map(Some(1))?;
                state.serialize_entry(key, hash)?;
                state.end()
            }
            Branch(ref branch) => branch.serialize(serializer),
        }
    }
}
impl<V: Serialize> Serialize for BranchProofNode<V> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use self::BranchProofNode::*;
        let mut state = serializer.serialize_map(Some(2))?;
        match *self {
            BranchKeyNotFound {
                left_hash: ref lhash,
                right_hash: ref rhash,
                left_key: ref lkey,
                right_key: ref rkey,
            } => {
                state.serialize_entry(lkey, lhash)?;
                state.serialize_entry(rkey, rhash)?;
            }
            LeftBranch {
                left_node: ref proof,
                right_hash: ref rhash,
                left_key: ref lkey,
                right_key: ref rkey,
            } => {
                state.serialize_entry(lkey, proof)?;
                state.serialize_entry(rkey, rhash)?;
            }
            RightBranch {
                left_hash: ref lhash,
                right_node: ref proof,
                left_key: ref lkey,
                right_key: ref rkey,
            } => {
                state.serialize_entry(lkey, lhash)?;
                state.serialize_entry(rkey, proof)?;
            }
        }
        state.end()
    }
}
impl<V: Serialize> Serialize for ProofNode<V> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use self::ProofNode::*;
        match *self {
            Leaf(ref value) => {

                #[derive(Serialize)]
                struct SerializeHelper<'a, V: Serialize + 'a> {
                    val: &'a V,
                }
                let helper = SerializeHelper { val: value };
                helper.serialize(serializer)
            }
            Branch(ref branch) => branch.serialize(serializer),
        }
    }
}

impl<V: fmt::Debug + StorageValue> MapProof<V> {
    /// Verifies the correctness of the proof by the trusted root hash and the requested key.
    ///
    /// If the proof is valid and the requested key exists, `Ok(Some(&V))` is returned.
    /// If the proof is valid and the requested key does not exists, `Ok(None)` is returned.
    /// If the proof is invalid, `Err` is returned.
    pub fn validate<K: ProofMapKey>(&self, key: &K, root_hash: Hash) -> Result<Option<&V>, Error> {
        let searched_slice = DBKey::leaf(key);
        use self::MapProof::*;

        // if we inspect the topmost level of a proof
        let res: Option<&V> = match *self {
            Empty => None,
            LeafRootInclusive(ref root_db_key, ref val) => {
                let root_slice = root_db_key;
                if root_slice != &searched_slice {
                    return Err(Error::new(format!(
                        "Proof is inconsistent with searched key: \
                                                   {:?}. Proof: {:?}. ",
                        searched_slice,
                        self
                    )));
                }
                Some(val)
            }
            LeafRootExclusive(ref root_db_key, _) => {
                let root_slice = root_db_key;
                if root_slice == &searched_slice {
                    return Err(Error::new(format!(
                        "Proof is inconsistent with searched key: \
                                                   {:?}. Proof: {:?} ",
                        searched_slice,
                        self
                    )));
                }
                None
            }
            Branch(ref branch) => branch.validate(&searched_slice)?,
        };
        let proof_hash = self.root_hash();
        if proof_hash != root_hash {
            return Err(Error::new(format!(
                "The proof doesn't match the expected hash! \
                                           Expected: {:?} , from proof: {:?}",
                root_hash,
                proof_hash
            )));
        }
        Ok(res)
    }
}

impl<V: fmt::Debug> BranchProofNode<V> {
    fn validate(&self, searched_slice: &DBKey) -> Result<Option<&V>, Error> {
        use self::BranchProofNode::*;

        // if we inspect the topmost level of a proof
        let res: Option<&V> = match *self {
            LeftBranch {
                left_node: ref proof,
                left_key: ref left_slice_key,
                ..
            } => {
                let left_slice = left_slice_key;
                if !searched_slice.starts_with(left_slice) {
                    return Err(Error::new(format!(
                        "Proof is inconsistent with searched_key: \
                                                   {:?}. Proof: {:?}",
                        searched_slice,
                        self
                    )));
                }
                proof.validate_consistency(left_slice, searched_slice)?
            }
            RightBranch {
                right_node: ref proof,
                right_key: ref right_slice_key,
                ..
            } => {
                let right_slice = right_slice_key;
                if !searched_slice.starts_with(right_slice) {
                    return Err(Error::new(format!(
                        "Proof is inconsistent with searched_key: \
                                                   {:?}. Proof: {:?}",
                        searched_slice,
                        self
                    )));
                }
                proof.validate_consistency(right_slice, searched_slice)?
            }
            BranchKeyNotFound {
                left_key: ref left_slice_key,
                right_key: ref right_slice_key,
                ..
            } => {
                let left_slice = left_slice_key;
                let right_slice = right_slice_key;
                if searched_slice.starts_with(left_slice) ||
                    searched_slice.starts_with(right_slice)
                {
                    return Err(Error::new(format!(
                        "Proof is inconsistent with searched_key: \
                                                   {:?}. Proof: {:?}",
                        searched_slice,
                        self
                    )));
                }
                None
            }
        };
        Ok(res)
    }

    fn validate_consistency<'a>(
        &'a self,
        parent_slice: &DBKey,
        searched_slice: &DBKey,
    ) -> Result<Option<&'a V>, Error> {
        use self::BranchProofNode::*;

        // if we inspect sub-proofs of a proof
        let res: Option<&V> = match *self {
            LeftBranch {
                left_node: ref proof,
                left_key: ref left_slice_key,
                right_key: ref right_slice_key,
                ..
            } => {
                let mut left_slice = left_slice_key.clone();
                left_slice.set_from(0);
                let mut right_slice = right_slice_key.clone();
                right_slice.set_from(0);
                if !left_slice.starts_with(parent_slice) || !right_slice.starts_with(parent_slice) {
                    return Err(Error::new(format!(
                        "Proof is inconsistent with itself: Proof: \
                                                   {:?} . Parent slice: {:?}",
                        self,
                        parent_slice
                    )));
                }
                if !searched_slice.starts_with(&left_slice) {
                    return Err(Error::new(format!(
                        "Proof is inconsistent with searched_key: \
                                                   {:?}. Proof: {:?}",
                        searched_slice,
                        self
                    )));
                }
                proof.validate_consistency(&left_slice, searched_slice)?
            }
            RightBranch {
                right_node: ref proof,
                left_key: ref left_slice_key,
                right_key: ref right_slice_key,
                ..
            } => {
                let mut left_slice = left_slice_key.clone();
                left_slice.set_from(0);
                let mut right_slice = right_slice_key.clone();
                right_slice.set_from(0);
                if !left_slice.starts_with(parent_slice) || !right_slice.starts_with(parent_slice) {
                    return Err(Error::new(format!(
                        "Proof is inconsistent with itself: Proof: \
                                                   {:?} . Parent slice: {:?}",
                        self,
                        parent_slice
                    )));
                }
                if !searched_slice.starts_with(&right_slice) {
                    return Err(Error::new(format!(
                        "Proof is inconsistent with searched_key: \
                                                   {:?}. Proof: {:?}",
                        searched_slice,
                        self
                    )));
                }
                proof.validate_consistency(&right_slice, searched_slice)?
            }
            BranchKeyNotFound {
                left_key: ref left_slice_key,
                right_key: ref right_slice_key,
                ..
            } => {
                let mut left_slice = left_slice_key.clone();
                left_slice.set_from(0);
                let mut right_slice = right_slice_key.clone();
                right_slice.set_from(0);
                if !left_slice.starts_with(parent_slice) || !right_slice.starts_with(parent_slice) {
                    return Err(Error::new(format!(
                        "Proof is inconsistent with itself: Proof: \
                                                   {:?} . Parent slice: {:?}",
                        self,
                        parent_slice
                    )));
                }
                if searched_slice.starts_with(&left_slice) ||
                    searched_slice.starts_with(&right_slice)
                {
                    return Err(Error::new(format!(
                        "Proof is inconsistent with searched_key: \
                                                   {:?}. Proof: {:?}",
                        searched_slice,
                        self
                    )));
                }
                None
            }
        };
        Ok(res)
    }
}
impl<V: fmt::Debug> ProofNode<V> {
    fn validate_consistency<'a>(
        &'a self,
        parent_slice: &DBKey,
        searched_slice: &DBKey,
    ) -> Result<Option<&'a V>, Error> {
        use self::ProofNode::*;

        // if we inspect sub-proofs of a proof
        let res: Option<&V> = match *self {
            Leaf(ref val) => {
                if searched_slice != parent_slice {
                    return Err(Error::new(format!(
                        "Proof is inconsistent with searched_key: \
                                                   {:?}. Parent slice: {:?} ",
                        searched_slice,
                        parent_slice
                    )));
                }
                Some(val)
            }
            Branch(ref branch) => branch.validate_consistency(parent_slice, searched_slice)?,
        };
        Ok(res)
    }
}

impl<V: fmt::Debug> fmt::Debug for MapProof<V> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::MapProof::*;
        match *self {
            LeafRootInclusive(ref db_key, ref val) => {
                write!(f, "{{\"slice\":{:?},{:?}}}", db_key, val)
            }
            LeafRootExclusive(ref db_key, ref val_hash) => {
                write!(f, "{{\"slice\":{:?},\"val_hash\":{:?}}}", db_key, val_hash)
            }
            Empty => write!(f, "{{}}"),
            Branch(ref branch) => write!(f, "{:?}", branch),
        }
    }
}
impl<V: fmt::Debug> fmt::Debug for ProofNode<V> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::ProofNode::*;
        match *self {
            Branch(ref branch) => write!(f, "{:?}", branch),
            Leaf(ref val) => write!(f, "{{\"val\":{:?}}}", val),
        }
    }
}

impl<V: fmt::Debug> fmt::Debug for BranchProofNode<V> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::BranchProofNode::*;
        match *self {
            LeftBranch {
                ref left_node,
                ref right_hash,
                ref left_key,
                ref right_key,
            } => {
                write!(
                    f,
                    "{{\"left\":{:?},\"right\":{:?},\"left_slice\":{:?},\"right_slice\":{:?}}}",
                    left_node,
                    right_hash,
                    left_key,
                    right_key
                )
            }
            RightBranch {
                ref left_hash,
                ref right_node,
                ref left_key,
                ref right_key,
            } => {
                write!(
                    f,
                    "{{\"left\":{:?},\"right\":{:?},\"left_slice\":{:?},\"right_slice\":{:?}}}",
                    left_hash,
                    right_node,
                    left_key,
                    right_key
                )
            }
            BranchKeyNotFound {
                ref left_hash,
                ref right_hash,
                ref left_key,
                ref right_key,
            } => {
                write!(
                    f,
                    "{{\"left\":{:?},\"right\":{:?},\"left_slice\":{:?},\"right_slice\":{:?}}}",
                    left_hash,
                    right_hash,
                    left_key,
                    right_key
                )
            }
        }
    }
}
