use ::storage::{StorageValue, Error};
use ::storage::utils::bytes_to_hex;
use ::storage::fields::{repr_stor_val, decode_from_b64_string};
use ::crypto::{hash, Hash};
use std::fmt;
use super::{BitSlice, KEY_SIZE};
use serde::{Serialize, Serializer};
use serde_json::Value;
const LEFT_HASH_DESC: &'static str = "left_hash";
const RIGHT_HASH_DESC: &'static str = "right_hash";
const LEFT_SLICE_DESC: &'static str = "left_key";
const RIGHT_SLICE_DESC: &'static str = "right_key";
const VAL_DESC: &'static str = "val";
const ROOT_KEY_DESC: &'static str = "root_key";
const ROOT_VAL_HASH: &'static str = "hash";


pub enum ProofPathToKey<V: StorageValue> {
    LeafRootInclusive(Vec<u8>, V), /* to match a leaf root with found key; (root_db_key= searched_db_key, value) */
    LeafRootExclusive(Vec<u8>, Hash), /* to prove exclusion for a leaf root when root_db_key != searched db_key */

    // left_hash, right_hash, left_slice_db_key, right_slice_db_key
    BranchKeyNotFound(Hash, Hash, Vec<u8>, Vec<u8>), /* to prove exclusion for a branch with both child_key(s) != prefix(searched_key) */
    // proof, right_slice_hash, left_slice_db_key, right_slice_db_key
    LeftBranch(Box<ProofPathToKey<V>>, Hash, Vec<u8>, Vec<u8>),
    // left_slice_hash, proof, left_slice_db_key, right_slice_db_key
    RightBranch(Hash, Box<ProofPathToKey<V>>, Vec<u8>, Vec<u8>),
    Leaf(V), // to prove inclusion of a value under searched_key below root level
}

impl<V: StorageValue + Clone> Serialize for ProofPathToKey<V> {
    fn serialize<S>(&self, ser: &mut S) -> Result<(), S::Error>
        where S: Serializer
    {
        use self::ProofPathToKey::*;
        let mut state;
        match *self {
            LeafRootInclusive(ref key, ref value) => {
                state = ser.serialize_struct("LeafRootInclusive", 2)?;
                ser.serialize_struct_elt(&mut state, ROOT_KEY_DESC, repr_stor_val(key))?;
                ser.serialize_struct_elt(&mut state, VAL_DESC, repr_stor_val(value))?;
            } 
            LeafRootExclusive(ref key, ref hash) => {
                state = ser.serialize_struct("LeafRootExclusive", 2)?;
                ser.serialize_struct_elt(&mut state, ROOT_KEY_DESC, repr_stor_val(key))?;
                ser.serialize_struct_elt(&mut state, ROOT_VAL_HASH, repr_stor_val(hash))?;
            } 
            BranchKeyNotFound(ref lhash, ref rhash, ref lkey, ref rkey) => {
                state = ser.serialize_struct("BranchKeyNotFound", 4)?;
                ser.serialize_struct_elt(&mut state, LEFT_HASH_DESC, repr_stor_val(lhash))?;
                ser.serialize_struct_elt(&mut state, RIGHT_HASH_DESC, repr_stor_val(rhash))?;
                ser.serialize_struct_elt(&mut state, LEFT_SLICE_DESC, repr_stor_val(lkey))?;
                ser.serialize_struct_elt(&mut state, RIGHT_SLICE_DESC, repr_stor_val(rkey))?;
            } 
            LeftBranch(ref proof, ref rhash, ref lkey, ref rkey) => {
                state = ser.serialize_struct("LeftBranch", 4)?;
                ser.serialize_struct_elt(&mut state, LEFT_HASH_DESC, proof)?;
                ser.serialize_struct_elt(&mut state, RIGHT_HASH_DESC, repr_stor_val(rhash))?;
                ser.serialize_struct_elt(&mut state, LEFT_SLICE_DESC, repr_stor_val(lkey))?;
                ser.serialize_struct_elt(&mut state, RIGHT_SLICE_DESC, repr_stor_val(rkey))?;
            }
            RightBranch(ref lhash, ref proof, ref lkey, ref rkey) => {
                state = ser.serialize_struct("RightBranch", 4)?;
                ser.serialize_struct_elt(&mut state, LEFT_HASH_DESC, repr_stor_val(lhash))?;
                ser.serialize_struct_elt(&mut state, RIGHT_HASH_DESC, proof)?;
                ser.serialize_struct_elt(&mut state, LEFT_SLICE_DESC, repr_stor_val(lkey))?;
                ser.serialize_struct_elt(&mut state, RIGHT_SLICE_DESC, repr_stor_val(rkey))?;
            } 
            Leaf(ref value) => {
                state = ser.serialize_struct("Leaf", 1)?;
                ser.serialize_struct_elt(&mut state, VAL_DESC, repr_stor_val(value))?;
            }
        }
        ser.serialize_struct_end(state)
    }
}



/// Returnes Ok(Some(Value)), if the proof proves inclusion of the Value in the `MerklePatriciaTable` for `the searched_key`
/// Ok(None): if it proves that the `searched_key` is excluded from the `MerklePatriciaTable`
/// Err(Error): if it's inconsistent a) with `root_hash` (its hash doesn't match the `root_hash`)
///                                 b) its structure is inconsistent with `searched_key`
///                                 c) its structure is inconsistent with itself (invalid enum variants are met or inconsistent parent and child bitslices)
#[allow(dead_code)]
pub fn verify_proof_consistency<V: StorageValue + fmt::Debug, A: AsRef<[u8]>>
    (proof: &ProofPathToKey<V>,
     searched_key: A,
     root_hash: Hash)
     -> Result<Option<&V>, Error> {
    let searched_key = searched_key.as_ref();
    debug_assert_eq!(searched_key.len(), KEY_SIZE);
    let searched_slice = BitSlice::from_bytes(searched_key);
    let result = proof.verify_root_proof_consistency(&searched_slice)?;

    let proof_hash = proof.compute_proof_root();
    if proof_hash != root_hash {
        return Err(Error::new(format!("The proof doesn't match the expected hash! Expected: \
                                       {:?} , from proof: {:?}",
                                      root_hash,
                                      proof_hash)));
    }
    Ok(result)
}

impl<V: StorageValue> ProofPathToKey<V> {
    pub fn deserialize(json: &Value) -> Result<Self, Error> {
        if !json.is_object() {
            return Err(Error::new(format!("Invalid json: it is expected to be json Object. \
                                           json: {:?}",
                                          json)));
        }
        let map_key_value = json.as_object().unwrap();
        let res: Self = match map_key_value.len() {
            1 => {
                if map_key_value.get(VAL_DESC).is_none() {
                    return Err(Error::new(format!("Invalid json: Key {} not found. Value: {:?}",
                                                  VAL_DESC,
                                                  json)));
                }
                let leaf_value = map_key_value.get(VAL_DESC).unwrap();
                if !leaf_value.is_string() {
                    return Err(Error::new(format!("Invalid json: leaf value is expected to be \
                                                   a string. json: {:?}",
                                                  leaf_value)));
                }
                let val_repr = leaf_value.as_str().unwrap();
                let val: V = decode_from_b64_string(val_repr).map_err(|e| {
                        Error::new(format!("Base64Error: {}. The value, that was attempted to be \
                                            decoded: {}",
                                           e,
                                           val_repr))
                    })?;
                ProofPathToKey::Leaf(val)  // only the VAL_DESC variant
            } 
            2 => {
                // either a [ROOT_KEY_DESC, VAL_DESC] or [ROOT_KEY_DESC, ROOT_VAL_HASH] variants
                if map_key_value.get(ROOT_KEY_DESC).is_none() {
                    return Err(Error::new(format!("Invalid json: Key {} not found. Value: {:?}",
                                                  ROOT_KEY_DESC,
                                                  json)));
                }
                if map_key_value.get(VAL_DESC).is_none() &&
                   map_key_value.get(ROOT_VAL_HASH).is_none() {
                    return Err(Error::new(format!("Invalid json: unknown key met. Expected: {} \
                                                   or {}. json: {:?}",
                                                  VAL_DESC,
                                                  ROOT_VAL_HASH,
                                                  json)));
                }
                let root_key_value = map_key_value.get(ROOT_KEY_DESC).unwrap();
                if !root_key_value.is_string() {
                    return Err(Error::new(format!("Invalid json: root_key is expected to \
                                                       be a string. json: {:?}",
                                                  root_key_value)));
                }
                let val_repr = root_key_value.as_str().unwrap();
                let root_key: Vec<u8> = decode_from_b64_string(val_repr).map_err(|e| {
                        Error::new(format!("Base64Error: {}. The value, that was attempted to be \
                                            decoded: {}",
                                           e,
                                           val_repr))
                    })?;


                if let Some(leaf_value) = map_key_value.get(VAL_DESC) {
                    if !leaf_value.is_string() {
                        return Err(Error::new(format!("Invalid json: leaf value is expected to \
                                                       be a string. json: {:?}",
                                                      leaf_value)));
                    }
                    let val_repr = leaf_value.as_str().unwrap();
                    let val: V = decode_from_b64_string(val_repr).map_err(|e| {
                            Error::new(format!("Base64Error: {}. The value, that was attempted \
                                                to be decoded: {}",
                                               e,
                                               val_repr))
                        })?;
                    ProofPathToKey::LeafRootInclusive(root_key, val)
                } else {
                    // ROOT_VAL_HASH is present
                    let hash_value = map_key_value.get(ROOT_VAL_HASH).unwrap();
                    if !hash_value.is_string() {
                        return Err(Error::new(format!("Invalid json: leaf value is expected to \
                                                       be a string. json: {:?}",
                                                      hash_value)));
                    }
                    let val_repr = hash_value.as_str().unwrap();
                    let hash: Hash = decode_from_b64_string(val_repr).map_err(|e| {
                            Error::new(format!("Base64Error: {}. The value, that was attempted \
                                                to be decoded: {}",
                                               e,
                                               val_repr))
                        })?;
                    ProofPathToKey::LeafRootExclusive(root_key, hash)
                }
            } 
            4 => {
                if map_key_value.get(LEFT_HASH_DESC).is_none() ||
                   map_key_value.get(RIGHT_HASH_DESC).is_none() ||
                   map_key_value.get(LEFT_SLICE_DESC).is_none() ||
                   map_key_value.get(RIGHT_SLICE_DESC).is_none() {
                    return Err(Error::new(format!("Invalid json: unknown key met. Expected: \
                                                   {}, {}, {} and {}. json: {:?}",
                                                  LEFT_HASH_DESC,
                                                  RIGHT_HASH_DESC,
                                                  LEFT_SLICE_DESC,
                                                  RIGHT_SLICE_DESC,
                                                  json)));
                }
                let (left_hash_value, right_hash_value, left_slice_value, right_slice_value) =
                    (map_key_value.get(LEFT_HASH_DESC).unwrap(),
                     map_key_value.get(RIGHT_HASH_DESC).unwrap(),
                     map_key_value.get(LEFT_SLICE_DESC).unwrap(),
                     map_key_value.get(RIGHT_SLICE_DESC).unwrap());
                if !left_slice_value.is_string() || !right_slice_value.is_string() {
                    return Err(Error::new(format!("Invalid json: both slice values are \
                                                   expected to be a strings. json1: {:?}, \
                                                   json2: {:?}",
                                                  left_slice_value,
                                                  right_slice_value)));
                }
                let val_repr = left_slice_value.as_str().unwrap();
                let left_slice: Vec<u8> = decode_from_b64_string(val_repr).map_err(|e| {
                        Error::new(format!("Base64Error: {}. The value, that was attempted to be \
                                            decoded: {}",
                                           e,
                                           val_repr))
                    })?;
                let val_repr = right_slice_value.as_str().unwrap();
                let right_slice: Vec<u8> = decode_from_b64_string(val_repr).map_err(|e| {
                        Error::new(format!("Base64Error: {}. The value, that was attempted to be \
                                            decoded: {}",
                                           e,
                                           val_repr))
                    })?;
                if !left_hash_value.is_string() && !right_hash_value.is_string() {
                    return Err(Error::new(format!("Invalid json: at least 1 of hash_value is \
                                                   expected to be a string. json1: {:?}, \
                                                   json2: {:?}",
                                                  left_slice_value,
                                                  right_slice_value)));
                }
                if left_hash_value.is_string() && right_hash_value.is_string() {
                    let val_repr = left_hash_value.as_str().unwrap();
                    let left_hash: Hash = decode_from_b64_string(val_repr).map_err(|e| {
                            Error::new(format!("Base64Error: {}. The value, that was attempted \
                                                to be decoded: {}",
                                               e,
                                               val_repr))
                        })?;
                    let val_repr = right_hash_value.as_str().unwrap();
                    let right_hash: Hash = decode_from_b64_string(val_repr).map_err(|e| {
                            Error::new(format!("Base64Error: {}. The value, that was attempted \
                                                to be decoded: {}",
                                               e,
                                               val_repr))
                        })?;
                    ProofPathToKey::BranchKeyNotFound(left_hash,
                                                      right_hash,
                                                      left_slice,
                                                      right_slice)
                } else if left_hash_value.is_string() {
                    let val_repr = left_hash_value.as_str().unwrap();
                    let left_hash: Hash = decode_from_b64_string(val_repr).map_err(|e| {
                            Error::new(format!("Base64Error: {}. The value, that was attempted \
                                                to be decoded: {}",
                                               e,
                                               val_repr))
                        })?;
                    let right_proof = Self::deserialize(right_hash_value)?;
                    ProofPathToKey::RightBranch(left_hash,
                                                Box::new(right_proof),
                                                left_slice,
                                                right_slice)
                } else {
                    // it's implied that right_hash_value.is_string() is true
                    let val_repr = right_hash_value.as_str().unwrap();
                    let right_hash: Hash = decode_from_b64_string(val_repr).map_err(|e| {
                            Error::new(format!("Base64Error: {}. The value, that was attempted \
                                                to be decoded: {}",
                                               e,
                                               val_repr))
                        })?;
                    let left_proof = Self::deserialize(left_hash_value)?;
                    ProofPathToKey::LeftBranch(Box::new(left_proof),
                                               right_hash,
                                               left_slice,
                                               right_slice)
                }
            } 
            _ => {
                return Err(Error::new(format!("Invalid json: Number of keys should be either 1, \
                                               2 or 4. json: {:?}",
                                              json)))
            }
        };
        Ok(res)
    }


    pub fn compute_proof_root(&self) -> Hash {
        use self::ProofPathToKey::*;
        match *self { 
            LeafRootInclusive(ref root_key, ref root_val) => {
                hash(&[root_key.as_slice(), root_val.hash().as_ref()].concat())
            } 
            LeafRootExclusive(ref root_key, ref root_val_hash) => {
                hash(&[root_key.as_slice(), root_val_hash.as_ref()].concat())
            } 
            BranchKeyNotFound(ref l_h, ref r_h, ref l_s, ref r_s) => {
                let full_slice = &[l_h.as_ref(), r_h.as_ref(), l_s.as_slice(), r_s.as_slice()]
                    .concat();
                hash(full_slice)
            }  
            LeftBranch(ref l_proof, ref right_hash, ref l_s, ref r_s) => {
                let full_slice = &[l_proof.compute_proof_root().as_ref(),
                                   right_hash.as_ref(),
                                   l_s.as_slice(),
                                   r_s.as_slice()]
                    .concat();
                hash(full_slice)
            } 
            RightBranch(ref left_hash, ref r_proof, ref l_s, ref r_s) => {
                let full_slice = &[left_hash.as_ref(),
                                   r_proof.compute_proof_root().as_ref(),
                                   l_s.as_slice(),
                                   r_s.as_slice()]
                    .concat();
                hash(full_slice)
            } 
            Leaf(ref val) => val.hash(),            
        }
    }

    pub fn compute_height(&self, start_height: u16) -> u16 {
        use self::ProofPathToKey::*;
        match *self { 
            LeafRootInclusive(_, _) |
            LeafRootExclusive(_, _) |
            BranchKeyNotFound(_, _, _, _) |
            Leaf(_) => start_height, 

            LeftBranch(ref l_proof, _, _, _) => l_proof.compute_height(start_height + 1), 

            RightBranch(_, ref r_proof, _, _) => r_proof.compute_height(start_height + 1),         
        }
    }
}

impl<V: StorageValue + fmt::Debug> ProofPathToKey<V> {
    fn verify_root_proof_consistency(&self,
                                     searched_slice: &BitSlice)
                                     -> Result<Option<&V>, Error> {
        use self::ProofPathToKey::*;

        // if we inspect the topmost level of a proof
        let res: Option<&V> = match *self {
            LeafRootInclusive(ref root_db_key, ref root_val) => {
                let root_slice = BitSlice::from_db_key(root_db_key);
                if root_slice != *searched_slice {
                    return Err(Error::new(format!("Proof is inconsistent with searched_key: \
                                                   {:?}. Proof: {:?}. ",
                                                  searched_slice,
                                                  self)));
                }
                Some(root_val)
            } 
            LeafRootExclusive(ref root_db_key, _) => {
                let root_slice = BitSlice::from_db_key(root_db_key);
                if root_slice == *searched_slice {
                    return Err(Error::new(format!("Proof is inconsistent with searched_key: \
                                                   {:?}. Proof: {:?} ",
                                                  searched_slice,
                                                  self)));
                }
                None
            } 
            Leaf(_) => {
                return Err(Error::new(format!("Invalid proof: Leaf enum variant found at top \
                                               level. Proof: {:?}",
                                              self)))
            } 

            LeftBranch(ref proof, _, ref left_slice_key, _) => {
                let left_slice = BitSlice::from_db_key(left_slice_key);
                if !searched_slice.starts_with(&left_slice) {
                    return Err(Error::new(format!("Proof is inconsistent with searched_key: \
                                                   {:?}. Proof: {:?}",
                                                  searched_slice,
                                                  self)));
                }
                proof.verify_proof_consistency(left_slice, searched_slice)?
            } 
            RightBranch(_, ref proof, _, ref right_slice_key) => {
                let right_slice = BitSlice::from_db_key(right_slice_key);
                if !searched_slice.starts_with(&right_slice) {
                    return Err(Error::new(format!("Proof is inconsistent with searched_key: \
                                                   {:?}. Proof: {:?}",
                                                  searched_slice,
                                                  self)));
                }
                proof.verify_proof_consistency(right_slice, searched_slice)?
            } 
            BranchKeyNotFound(_, _, ref left_slice_key, ref right_slice_key) => {
                let left_slice = BitSlice::from_db_key(left_slice_key);
                let right_slice = BitSlice::from_db_key(right_slice_key);
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

    fn verify_proof_consistency<'a, 'c>(&'a self,
                                        parent_slice: BitSlice<'c>,
                                        searched_slice: &BitSlice<'c>)
                                        -> Result<Option<&'a V>, Error> {
        use self::ProofPathToKey::*;

        // if we inspect sub-proofs of a proof
        let res: Option<&V> = match *self {
            LeafRootInclusive(_, _) => {
                return Err(Error::new(format!("Invalid proof: LeafRootInclusive enum variant \
                                               found not at top level. Proof: {:?}",
                                              self)))
            } 
            LeafRootExclusive(_, _) => {
                return Err(Error::new(format!("Invalid proof: LeafRootExclusive enum variant \
                                               found not at top level. Proof: {:?}",
                                              self)))
            } 
            Leaf(ref val) => {
                if (*searched_slice) != parent_slice {
                    return Err(Error::new(format!("Proof is inconsistent with searched_key: \
                                                   {:?}. Parent slice: {:?} ",
                                                  searched_slice,
                                                  parent_slice)));
                }
                Some(val)
            } 
            LeftBranch(ref proof, _, ref left_slice_key, ref right_slice_key) => {
                let left_slice = BitSlice::from_db_key(left_slice_key);
                let right_slice = BitSlice::from_db_key(right_slice_key);
                if !left_slice.starts_with(&parent_slice) ||
                   !right_slice.starts_with(&parent_slice) {
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
                proof.verify_proof_consistency(left_slice, searched_slice)?
            } 
            RightBranch(_, ref proof, ref left_slice_key, ref right_slice_key) => {
                let left_slice = BitSlice::from_db_key(left_slice_key);
                let right_slice = BitSlice::from_db_key(right_slice_key);
                if !left_slice.starts_with(&parent_slice) ||
                   !right_slice.starts_with(&parent_slice) {
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
                proof.verify_proof_consistency(right_slice, searched_slice)?
            } 
            BranchKeyNotFound(_, _, ref left_slice_key, ref right_slice_key) => {
                let left_slice = BitSlice::from_db_key(left_slice_key);
                let right_slice = BitSlice::from_db_key(right_slice_key);
                if !left_slice.starts_with(&parent_slice) ||
                   !right_slice.starts_with(&parent_slice) {
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

impl<V: StorageValue + fmt::Debug> fmt::Debug for ProofPathToKey<V> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::ProofPathToKey::*;
        match *self {
            LeftBranch(ref proof, ref hash, ref left_slice_key, ref right_slice_key) => {
                write!(f,
                       "{{ left: {:?}, right: {:?}, left_slice: {:?},  right_slice: {:?} }}",
                       proof,
                       bytes_to_hex(hash),
                       BitSlice::from_db_key(left_slice_key),
                       BitSlice::from_db_key(right_slice_key))
            } 
            RightBranch(ref hash, ref proof, ref left_slice_key, ref right_slice_key) => {
                write!(f,
                       "{{ left: {:?}, right: {:?}, left_slice: {:?},  right_slice: {:?} }}",
                       bytes_to_hex(hash),
                       proof,
                       BitSlice::from_db_key(left_slice_key),
                       BitSlice::from_db_key(right_slice_key))
            } 
            Leaf(ref val) => write!(f, "{{ val: {:?} }}", val), 
            BranchKeyNotFound(ref l_hash, ref r_hash, ref left_slice_key, ref right_slice_key) => {
                write!(f,
                       "{{left: {:?}, right: {:?}, left_slice: {:?},  \
                        right_slice: {:?} }}",
                       bytes_to_hex(l_hash),
                       bytes_to_hex(r_hash),
                       BitSlice::from_db_key(left_slice_key),
                       BitSlice::from_db_key(right_slice_key))
            }
            LeafRootInclusive(ref db_key, ref val) => {
                write!(f,
                       "{{ slice: {:?}, val: {:?} }}",
                       BitSlice::from_db_key(db_key),
                       val)
            } 
            LeafRootExclusive(ref db_key, ref val_hash) => {
                write!(f,
                       "{{ slice: {:?}, val_hash: {:?} }}",
                       BitSlice::from_db_key(db_key),
                       bytes_to_hex(val_hash))
            }
        }
    }
}
