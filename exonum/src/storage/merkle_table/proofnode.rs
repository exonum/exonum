use ::storage::{StorageValue, Error};
use ::storage::utils::bytes_to_hex;
use ::storage::fields::{repr_stor_val, decode_from_b64_string};
use ::crypto::{hash, Hash};
use std::fmt;
use serde::{Serialize, Serializer};
use serde_json::Value;
const LEFT_DESC: &'static str = "left";
const RIGHT_DESC: &'static str = "rigth";
const VAL_DESC: &'static str = "val";
#[allow(dead_code)]
pub fn proof_indices_values<V: StorageValue>(proof: &Proofnode<V>) -> Vec<(usize, &V)> {
    let mut res = Vec::new();
    proof.indices_and_values(0usize, &mut res);
    res
}

pub enum Proofnode<V: StorageValue> {
    Full(Box<Proofnode<V>>, Box<Proofnode<V>>),
    Left(Box<Proofnode<V>>, Option<Hash>),
    Right(Hash, Box<Proofnode<V>>),
    Leaf(V),
}

impl<V: StorageValue> Serialize for Proofnode<V> {
    fn serialize<S>(&self, ser: &mut S) -> Result<(), S::Error>
        where S: Serializer
    {
        use self::Proofnode::*;
        let mut state;
        match *self {
            Full(ref left_proof, ref right_proof) => {
                state = ser.serialize_struct("Full", 2)?;
                ser.serialize_struct_elt(&mut state, LEFT_DESC, left_proof)?;
                ser.serialize_struct_elt(&mut state, RIGHT_DESC, right_proof)?;
            } 
            Left(ref left_proof, ref option_hash) => {
                if let Some(ref hash) = *option_hash {
                    state = ser.serialize_struct("Left", 2)?;
                    let hash_repr = repr_stor_val(hash);
                    ser.serialize_struct_elt(&mut state, LEFT_DESC, left_proof)?;
                    ser.serialize_struct_elt(&mut state, RIGHT_DESC, hash_repr)?;
                } else {
                    state = ser.serialize_struct("Left", 1)?;
                    ser.serialize_struct_elt(&mut state, LEFT_DESC, left_proof)?;
                }
            } 
            Right(ref hash, ref right_proof) => {
                state = ser.serialize_struct("Right", 2)?;
                let hash_repr = repr_stor_val(hash);
                ser.serialize_struct_elt(&mut state, LEFT_DESC, hash_repr)?;
                ser.serialize_struct_elt(&mut state, RIGHT_DESC, right_proof)?;
            } 
            Leaf(ref val) => {
                state = ser.serialize_struct("Leaf", 1)?;
                let val_repr = repr_stor_val(val);
                ser.serialize_struct_elt(&mut state, VAL_DESC, val_repr)?;
            }
        }
        ser.serialize_struct_end(state)
    }
}

impl<V: StorageValue> Proofnode<V> {
    pub fn deserialize(json: &Value) -> Result<Self, Error> {
        if !json.is_object() {
            return Err(Error::new(format!("Invalid json: it is expected to be json Object. \
                                           json: {:?}",
                                          json)));
        }
        let map_key_value = json.as_object().unwrap();
        let res: Self = match map_key_value.len() {
            2 => {
                let left_value: &Value = match map_key_value.get(LEFT_DESC) {
                    None => {
                        return Err(Error::new(format!("Invalid json: Key {} not found. Value: \
                                                       {:?}",
                                                      LEFT_DESC,
                                                      json)))
                    } 
                    Some(left) => left, 
                };
                let right_value: &Value = match map_key_value.get(RIGHT_DESC) {
                    None => {
                        return Err(Error::new(format!("Invalid json: Key {} not found. Value: \
                                                       {:?}",
                                                      RIGHT_DESC,
                                                      json)))
                    } 
                    Some(right) => right, 
                };
                if right_value.is_string() {
                    let left_proof = Self::deserialize(left_value)?;
                    let val_repr = right_value.as_str().unwrap();
                    let right_hash: Hash = decode_from_b64_string(val_repr).map_err(|e| {
                            Error::new(format!("Base64Error: {}. The value, that was attempted \
                                                to be decoded: {}",
                                               e,
                                               val_repr))
                        })?;
                    Proofnode::Left(Box::new(left_proof), Some(right_hash))
                } else if left_value.is_string() {
                    let right_proof = Self::deserialize(right_value)?;
                    let val_repr = left_value.as_str().unwrap();
                    let left_hash: Hash = decode_from_b64_string(val_repr).map_err(|e| {
                            Error::new(format!("Base64Error: {}. The value, that was attempted \
                                                to be decoded: {}",
                                               e,
                                               val_repr))
                        })?;
                    Proofnode::Right(left_hash, Box::new(right_proof))
                } else {
                    let left_proof = Self::deserialize(left_value)?;
                    let right_proof = Self::deserialize(right_value)?;
                    Proofnode::Full(Box::new(left_proof), Box::new(right_proof))
                }
            } 
            1 => {
                if map_key_value.get(VAL_DESC).is_none() && map_key_value.get(LEFT_DESC).is_none() {
                    return Err(Error::new(format!("Invalid json: unknown key met. Expected: {} \
                                                   or {}. json: {:?}",
                                                  VAL_DESC,
                                                  LEFT_DESC,
                                                  json)));
                }
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
                    Proofnode::Leaf(val)

                } else {
                    // LEFT_DESC is present
                    let left_proof_value = map_key_value.get(LEFT_DESC).unwrap();
                    let left_proof = Self::deserialize(left_proof_value)?;
                    Proofnode::Left(Box::new(left_proof), None)
                }
            } 
            _ => {
                return Err(Error::new(format!("Invalid json: Number of keys should be either 1 \
                                               or 2. json: {:?}",
                                              json)))
            } 
        };
        Ok(res)
    }

    pub fn compute_proof_root(&self) -> Hash {
        match *self {
            Proofnode::Full(ref left, ref right) => {
                hash(&[left.compute_proof_root().as_ref(), right.compute_proof_root().as_ref()]
                    .concat())
            }
            Proofnode::Left(ref left_proof, ref right_hash) => {
                if let Some(ref hash_val) = *right_hash {
                    hash(&[left_proof.compute_proof_root().as_ref(), hash_val.as_ref()].concat())
                } else {
                    hash(left_proof.compute_proof_root().as_ref())
                }
            } 
            Proofnode::Right(ref left_hash, ref right_proof) => {
                hash(&[left_hash.as_ref(), right_proof.compute_proof_root().as_ref()].concat())
            }
            Proofnode::Leaf(ref val) => val.hash(), 
        }
    }

    fn indices_and_values<'a>(&'a self, index: usize, collect: &mut Vec<(usize, &'a V)>) {
        let left_ch_ind = index * 2;
        let right_ch_ind = index * 2 + 1;
        match *self {
            Proofnode::Full(ref left, ref right) => {
                left.indices_and_values(left_ch_ind, collect);
                right.indices_and_values(right_ch_ind, collect);
            }
            Proofnode::Left(ref left_proof, _) => {
                left_proof.indices_and_values(left_ch_ind, collect);
            } 
            Proofnode::Right(_, ref right_proof) => {
                right_proof.indices_and_values(right_ch_ind, collect);
            }
            Proofnode::Leaf(ref val) => collect.push((index, val)),
        }
    }
}

impl<V: StorageValue + fmt::Debug> fmt::Debug for Proofnode<V> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::Proofnode::*;
        match *self {
            Full(ref left, ref right) => write!(f, "{{ left: {:?}, right: {:?} }}", left, right),
            Left(ref left_proof, ref right_hash) => {
                let hash_repr: String;
                if let Some(ref digest) = *right_hash {
                    hash_repr = bytes_to_hex(digest);
                } else {
                    hash_repr = "None".to_string();
                }
                write!(f,
                       "{{ left: {:?}, right_hash: {:?} }}",
                       left_proof,
                       hash_repr)
            } 
            Right(ref left_hash, ref right) => {
                let hash_repr: String;
                hash_repr = bytes_to_hex(left_hash);
                write!(f, "{{ left_hash: {:?}, right: {:?} }}", hash_repr, right)
            }
            Leaf(ref val) => write!(f, "{{ val: {:?} }}", val), 
        }
    }
}
