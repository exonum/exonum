use serde::{Serialize, Serializer, Deserialize, Deserializer};
use serde::de::Error;
use serde_json::{Error as SerdeJsonError, Value, from_value};

use std::fmt;

use storage::StorageValue;
use crypto::Hash;
use super::hash_rules;

const LEFT_DESC: &'static str = "left";
const RIGHT_DESC: &'static str = "right";
const VAL_DESC: &'static str = "val";

#[allow(dead_code)]
pub fn proof_indices_values<V: StorageValue>(proof: &Proofnode<V>) -> Vec<(usize, &V)> {
    let mut res = Vec::new();
    proof.indices_and_values(0usize, &mut res);
    res
}

pub enum Proofnode<V> {
    Full(Box<Proofnode<V>>, Box<Proofnode<V>>),
    Left(Box<Proofnode<V>>, Option<Hash>),
    Right(Hash, Box<Proofnode<V>>),
    Leaf(V),
}

impl<V: Serialize> Serialize for Proofnode<V> {
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
                    ser.serialize_struct_elt(&mut state, LEFT_DESC, left_proof)?;
                    ser.serialize_struct_elt(&mut state, RIGHT_DESC, hash)?;
                } else {
                    state = ser.serialize_struct("Left", 1)?;
                    ser.serialize_struct_elt(&mut state, LEFT_DESC, left_proof)?;
                }
            } 
            Right(ref hash, ref right_proof) => {
                state = ser.serialize_struct("Right", 2)?;
                ser.serialize_struct_elt(&mut state, LEFT_DESC, hash)?;
                ser.serialize_struct_elt(&mut state, RIGHT_DESC, right_proof)?;
            } 
            Leaf(ref val) => {
                state = ser.serialize_struct("Leaf", 1)?;
                ser.serialize_struct_elt(&mut state, VAL_DESC, val)?;
            }
        }
        ser.serialize_struct_end(state)
    }
}
impl<V: Deserialize> Deserialize for Proofnode<V> {
    fn deserialize<D>(deserializer: &mut D) -> Result<Self, D::Error>
        where D: Deserializer
    {
        fn format_err_string(type_str: &str, value: Value, err: SerdeJsonError) -> String {
            format!("Couldn't deserialize {} from serde_json::Value: {}, error: {}",
                    type_str,
                    value,
                    err)
        }

        let json: Value = <Value as Deserialize>::deserialize(deserializer)?;
        if !json.is_object() {
            return Err(D::Error::custom(format!("Invalid json: it is expected to be json \
                                                 Object. json: {:?}",
                                                json)));
        }
        let map_key_value = json.as_object().unwrap();
        let res: Self = match map_key_value.len() {
            2 => {
                let left_value: &Value = match map_key_value.get(LEFT_DESC) {
                    None => {
                        return Err(D::Error::custom(format!("Invalid json: Key {} not found. \
                                                             Value: {:?}",
                                                            LEFT_DESC,
                                                            json)))
                    } 
                    Some(left) => left, 
                };
                let right_value: &Value = match map_key_value.get(RIGHT_DESC) {
                    None => {
                        return Err(D::Error::custom(format!("Invalid json: Key {} not found. \
                                                          Value: {:?}",
                                                            RIGHT_DESC,
                                                            json)))
                    } 
                    Some(right) => right, 
                };
                if right_value.is_string() {
                    let left_proof: Proofnode<V> = from_value(left_value.clone()).map_err(|err| {
                            D::Error::custom(format_err_string("Proofnode",
                                                               left_value.clone(),
                                                               err))
                        })?;
                    let right_hash: Hash = from_value(right_value.clone()).map_err(|err| {
                            D::Error::custom(format_err_string("Hash", right_value.clone(), err))
                        })?;
                    Proofnode::Left(Box::new(left_proof), Some(right_hash))
                } else if left_value.is_string() {
                    let right_proof: Proofnode<V> = from_value(right_value.clone()).map_err(|err| {
                            D::Error::custom(format_err_string("Proofnode",
                                                               right_value.clone(),
                                                               err))
                        })?;
                    let left_hash: Hash = from_value(left_value.clone()).map_err(|err| {
                            D::Error::custom(format_err_string("Hash", left_value.clone(), err))
                        })?;
                    Proofnode::Right(left_hash, Box::new(right_proof))
                } else {
                    let left_proof = from_value(left_value.clone()).map_err(|err| {
                            D::Error::custom(format_err_string("Proofnode",
                                                               left_value.clone(),
                                                               err))
                        })?;
                    let right_proof = from_value(right_value.clone()).map_err(|err| {
                            D::Error::custom(format_err_string("Proofnode",
                                                               right_value.clone(),
                                                               err))
                        })?;
                    Proofnode::Full(Box::new(left_proof), Box::new(right_proof))
                }
            } 
            1 => {
                if map_key_value.get(VAL_DESC).is_none() && map_key_value.get(LEFT_DESC).is_none() {
                    return Err(D::Error::custom(format!("Invalid json: unknown key met. \
                                                         Expected: {} or {}. json: {:?}",
                                                        VAL_DESC,
                                                        LEFT_DESC,
                                                        json)));
                }
                if let Some(leaf_value) = map_key_value.get(VAL_DESC) {
                    let val: V = from_value(leaf_value.clone()).map_err(|err| {
                            D::Error::custom(format_err_string("V", leaf_value.clone(), err))
                        })?;
                    Proofnode::Leaf(val)
                } else {
                    // LEFT_DESC is present
                    let left_value = map_key_value.get(LEFT_DESC).unwrap();
                    let left_proof: Proofnode<V> = from_value(left_value.clone()).map_err(|err| {
                            D::Error::custom(format_err_string("Proofnode",
                                                               left_value.clone(),
                                                               err))
                        })?;
                    Proofnode::Left(Box::new(left_proof), None)
                }
            } 
            _ => {
                return Err(D::Error::custom(format!("Invalid json: Number of keys should be \
                                                     either 1 or 2. json: {:?}",
                                                    json)))
            } 
        };
        Ok(res)
    }
}

impl<V: StorageValue> Proofnode<V> {
    pub fn compute_proof_root(&self) -> Hash {
        match *self {
            Proofnode::Full(ref left, ref right) => {
                hash_rules::hash_branch(left.compute_proof_root(), right.compute_proof_root())
            }
            Proofnode::Left(ref left_proof, ref right_hash) => {
                if let Some(ref hash_val) = *right_hash {
                    hash_rules::hash_branch(left_proof.compute_proof_root(), *hash_val)
                } else {
                    hash_rules::hash_single_branch(left_proof.compute_proof_root())
                }
            } 
            Proofnode::Right(ref left_hash, ref right_proof) => {
                hash_rules::hash_branch(*left_hash, right_proof.compute_proof_root())
            }
            Proofnode::Leaf(ref val) => hash_rules::hash_leaf(val), 
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

impl<V: fmt::Debug> fmt::Debug for Proofnode<V> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::Proofnode::*;
        match *self {
            Full(ref left, ref right) => write!(f, "{{\"left\":{:?},\"right\":{:?}}}", left, right),
            Left(ref left_proof, ref right_hash) => {
                if let Some(ref digest) = *right_hash {
                    write!(f, "{{\"left\":{:?},\"right\":{:?}}}", left_proof, digest)
                } else {
                    write!(f, "{{\"left\":{:?}}}", left_proof)
                }
            } 
            Right(ref left_hash, ref right) => {
                write!(f, "{{\"left\":{:?},\"right\":{:?}}}", left_hash, right)
            }
            Leaf(ref val) => write!(f, "{{\"val\":{:?}}}", val), 
        }
    }
}
