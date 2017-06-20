use std::fmt;

use serde::{Serialize, Serializer};
use serde::ser::SerializeMap;

use crypto::{hash, Hash};

use super::super::{StorageValue, Error};
use super::key::{ProofMapKey, DBKey, ChildKind, KEY_SIZE};

impl Serialize for DBKey {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        let mut repr = String::with_capacity(KEY_SIZE * 8);
        let bslice = self;
        for ind in self.from()..self.to() {
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

pub enum MapProof<V> {
    /// to match a leaf root with found key; (root_db_key= searched_db_key, value)
    LeafRootInclusive(DBKey, V),
    /// to prove exclusion for a leaf root when root_db_key != searched db_key
    LeafRootExclusive(DBKey, Hash),
    Empty,
    Branch(BranchProofNode<V>),
}

pub enum ProofNode<V> {
    Branch(BranchProofNode<V>),
    /// to prove inclusion of a value under searched_key below root level
    Leaf(V),
}

pub enum BranchProofNode<V> {
    /// format: (left_hash, right_hash, left_slice_db_key, right_slice_db_key)
    /// to prove exclusion for a branch with both child_key(s) != prefix(searched_key)
    BranchKeyNotFound {
        left_hash: Hash,
        right_hash: Hash,
        left_key: DBKey,
        right_key: DBKey,
    },
    /// format: (proof, right_slice_hash, left_slice_db_key, right_slice_db_key)
    LeftBranch {
        left_hash: Box<ProofNode<V>>,
        right_hash: Hash,
        left_key: DBKey,
        right_key: DBKey,
    },
    /// format: (left_slice_hash, proof, left_slice_db_key, right_slice_db_key)
    RightBranch {
        left_hash: Hash,
        right_hash: Box<ProofNode<V>>,
        left_key: DBKey,
        right_key: DBKey,
    },
}

impl<V: StorageValue> MapProof<V> {
    pub fn compute_proof_root(&self) -> Hash {
        use self::MapProof::*;
        match *self {
            Empty => Hash::zero(),
            LeafRootInclusive(ref root_key, ref root_val) => {
                hash(&[&root_key.to_vec(), root_val.hash().as_ref()].concat())
            }
            LeafRootExclusive(ref root_key, ref root_val_hash) => {
                hash(&[&root_key.to_vec(), root_val_hash.as_ref()].concat())
            }
            Branch(ref branch) => branch.compute_proof_root(),
        }
    }
}
impl<V: StorageValue> ProofNode<V> {
    pub fn compute_proof_root(&self) -> Hash {
        use self::ProofNode::*;
        match *self {
            Leaf(ref val) => val.hash(),
            Branch(ref branch) => branch.compute_proof_root(),
        }
    }
}

impl<V: StorageValue> BranchProofNode<V> {
    pub fn compute_proof_root(&self) -> Hash {
        use self::BranchProofNode::*;
        match *self {
            BranchKeyNotFound {
                ref left_hash,
                ref right_hash,
                ref left_key,
                ref right_key,
            } => {
                let full_slice = &[left_hash.as_ref(),
                                   right_hash.as_ref(),
                                   &left_key.to_vec(),
                                   &right_key.to_vec()]
                                          .concat();
                hash(full_slice)
            }
            LeftBranch {
                ref left_hash,
                ref right_hash,
                ref left_key,
                ref right_key,
            } => {
                let full_slice = &[left_hash.compute_proof_root().as_ref(),
                                   right_hash.as_ref(),
                                   &left_key.to_vec(),
                                   &right_key.to_vec()]
                                          .concat();
                hash(full_slice)
            }
            RightBranch {
                ref left_hash,
                ref right_hash,
                ref left_key,
                ref right_key,
            } => {
                let full_slice = &[left_hash.as_ref(),
                                   right_hash.compute_proof_root().as_ref(),
                                   &left_key.to_vec(),
                                   &right_key.to_vec()]
                                          .concat();
                hash(full_slice)
            }
        }
    }
}


impl<V: Serialize> Serialize for MapProof<V> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer
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
        where S: Serializer
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
                left_hash: ref proof,
                right_hash: ref rhash,
                left_key: ref lkey,
                right_key: ref rkey,
            } => {
                state.serialize_entry(lkey, proof)?;
                state.serialize_entry(rkey, rhash)?;
            }
            RightBranch {
                left_hash: ref lhash,
                right_hash: ref proof,
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
        where S: Serializer
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

// impl<V> ProofPathToKey<V> {
//     pub fn compute_height(&self, start_height: u16) -> u16 {
//         use self::ProofPathToKey::*;
//         match *self {
//             LeafRootInclusive(..) |
//             LeafRootExclusive(..) |
//             BranchKeyNotFound(..) |
//             Leaf(..) |
//             Empty => start_height,

//             LeftBranch(ref l_proof, _, _, _) => l_proof.compute_height(start_height + 1),

//             RightBranch(_, ref r_proof, _, _) => r_proof.compute_height(start_height + 1),
//         }
//     }
// }

impl<V: fmt::Debug + StorageValue> MapProof<V> {
    pub fn verify_root_proof_consistency<K: ProofMapKey>(&self,
                                                         searched_key: &K,
                                                         root_hash: Hash)
                                                         -> Result<Option<&V>, Error> {
        let searched_slice = DBKey::leaf(searched_key);
        use self::MapProof::*;

        // if we inspect the topmost level of a proof
        let res: Option<&V> = match *self {
            Empty => None,
            LeafRootInclusive(ref root_db_key, ref val) => {
                let root_slice = root_db_key;
                if root_slice != &searched_slice {
                    return Err(Error::new(format!("Proof is inconsistent with searched_key: \
                                                   {:?}. Proof: {:?}. ",
                                                  searched_slice,
                                                  self)));
                }
                Some(val)
            }
            LeafRootExclusive(ref root_db_key, _) => {
                let root_slice = root_db_key;
                if root_slice == &searched_slice {
                    return Err(Error::new(format!("Proof is inconsistent with searched_key: \
                                                   {:?}. Proof: {:?} ",
                                                  searched_slice,
                                                  self)));
                }
                None
            }
            Branch(ref branch) => branch.verify_root_proof_consistency(&searched_slice)?,
        };
        let proof_hash = self.compute_proof_root();
        if proof_hash != root_hash {
            return Err(Error::new(format!("The proof doesn't match the expected hash! \
                                           Expected: {:?} , from proof: {:?}",
                                          root_hash,
                                          proof_hash)));
        }
        Ok(res)
    }
}

impl<V: fmt::Debug> BranchProofNode<V> {
    fn verify_root_proof_consistency(&self, searched_slice: &DBKey) -> Result<Option<&V>, Error> {
        use self::BranchProofNode::*;

        // if we inspect the topmost level of a proof
        let res: Option<&V> = match *self {
            LeftBranch {
                left_hash: ref proof,
                left_key: ref left_slice_key,
                ..
            } => {
                let left_slice = left_slice_key;
                if !searched_slice.starts_with(left_slice) {
                    return Err(Error::new(format!("Proof is inconsistent with searched_key: \
                                                   {:?}. Proof: {:?}",
                                                  searched_slice,
                                                  self)));
                }
                proof
                    .verify_proof_consistency(left_slice, searched_slice)?
            }
            RightBranch {
                right_hash: ref proof,
                right_key: ref right_slice_key,
                ..
            } => {
                let right_slice = right_slice_key;
                if !searched_slice.starts_with(right_slice) {
                    return Err(Error::new(format!("Proof is inconsistent with searched_key: \
                                                   {:?}. Proof: {:?}",
                                                  searched_slice,
                                                  self)));
                }
                proof
                    .verify_proof_consistency(right_slice, searched_slice)?
            }
            BranchKeyNotFound {
                left_key: ref left_slice_key,
                right_key: ref right_slice_key,
                ..
            } => {
                let left_slice = left_slice_key;
                let right_slice = right_slice_key;
                if searched_slice.starts_with(left_slice) ||
                   searched_slice.starts_with(right_slice) {
                    return Err(Error::new(format!("Proof is inconsistent with searched_key: \
                                                   {:?}. Proof: {:?}",
                                                  searched_slice,
                                                  self)));
                }
                None
            }
        };
        Ok(res)
    }

    fn verify_proof_consistency<'a>(&'a self,
                                    parent_slice: &DBKey,
                                    searched_slice: &DBKey)
                                    -> Result<Option<&'a V>, Error> {
        use self::BranchProofNode::*;

        // if we inspect sub-proofs of a proof
        let res: Option<&V> = match *self {
            LeftBranch {
                left_hash: ref proof,
                left_key: ref left_slice_key,
                right_key: ref right_slice_key,
                ..
            } => {
                let mut left_slice = left_slice_key.clone();
                left_slice.set_from(0);
                let mut right_slice = right_slice_key.clone();
                right_slice.set_from(0);
                if !left_slice.starts_with(parent_slice) ||
                   !right_slice.starts_with(parent_slice) {
                    return Err(Error::new(format!("Proof is inconsistent with itself: Proof: \
                                                   {:?} . Parent slice: {:?}",
                                                  self,
                                                  parent_slice)));
                }
                if !searched_slice.starts_with(&left_slice) {
                    return Err(Error::new(format!("Proof is inconsistent with searched_key: \
                                                   {:?}. Proof: {:?}",
                                                  searched_slice,
                                                  self)));
                }
                proof
                    .verify_proof_consistency(&left_slice, searched_slice)?
            }
            RightBranch {
                right_hash: ref proof,
                left_key: ref left_slice_key,
                right_key: ref right_slice_key,
                ..
            } => {
                let mut left_slice = left_slice_key.clone();
                left_slice.set_from(0);
                let mut right_slice = right_slice_key.clone();
                right_slice.set_from(0);
                if !left_slice.starts_with(parent_slice) ||
                   !right_slice.starts_with(parent_slice) {
                    return Err(Error::new(format!("Proof is inconsistent with itself: Proof: \
                                                   {:?} . Parent slice: {:?}",
                                                  self,
                                                  parent_slice)));
                }
                if !searched_slice.starts_with(&right_slice) {
                    return Err(Error::new(format!("Proof is inconsistent with searched_key: \
                                                   {:?}. Proof: {:?}",
                                                  searched_slice,
                                                  self)));
                }
                proof
                    .verify_proof_consistency(&right_slice, searched_slice)?
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
                if !left_slice.starts_with(parent_slice) ||
                   !right_slice.starts_with(parent_slice) {
                    return Err(Error::new(format!("Proof is inconsistent with itself: Proof: \
                                                   {:?} . Parent slice: {:?}",
                                                  self,
                                                  parent_slice)));
                }
                if searched_slice.starts_with(&left_slice) ||
                   searched_slice.starts_with(&right_slice) {
                    return Err(Error::new(format!("Proof is inconsistent with searched_key: \
                                                   {:?}. Proof: {:?}",
                                                  searched_slice,
                                                  self)));
                }
                None
            }
        };
        Ok(res)
    }
}
impl<V: fmt::Debug> ProofNode<V> {
    fn verify_proof_consistency<'a>(&'a self,
                                    parent_slice: &DBKey,
                                    searched_slice: &DBKey)
                                    -> Result<Option<&'a V>, Error> {
        use self::ProofNode::*;

        // if we inspect sub-proofs of a proof
        let res: Option<&V> = match *self {
            Leaf(ref val) => {
                if searched_slice != parent_slice {
                    return Err(Error::new(format!("Proof is inconsistent with searched_key: \
                                                   {:?}. Parent slice: {:?} ",
                                                  searched_slice,
                                                  parent_slice)));
                }
                Some(val)
            }
            Branch(ref branch) => {
                branch
                    .verify_proof_consistency(parent_slice, searched_slice)?
            }
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
                ref left_hash,
                ref right_hash,
                ref left_key,
                ref right_key,
            } => {
                write!(f,
                       "{{\"left\":{:?},\"right\":{:?},\"left_slice\":{:?},\"right_slice\":{:?}}}",
                       left_hash,
                       right_hash,
                       left_key,
                       right_key)
            }
            RightBranch {
                ref left_hash,
                ref right_hash,
                ref left_key,
                ref right_key,
            } => {
                write!(f,
                       "{{\"left\":{:?},\"right\":{:?},\"left_slice\":{:?},\"right_slice\":{:?}}}",
                       left_hash,
                       right_hash,
                       left_key,
                       right_key)
            }
            BranchKeyNotFound {
                ref left_hash,
                ref right_hash,
                ref left_key,
                ref right_key,
            } => {
                write!(f,
                       "{{\"left\":{:?},\"right\":{:?},\"left_slice\":{:?},\"right_slice\":{:?}}}",
                       left_hash,
                       right_hash,
                       left_key,
                       right_key)
            }
        }
    }
}
