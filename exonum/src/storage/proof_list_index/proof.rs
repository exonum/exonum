use serde::{Serialize, Serializer, Deserialize, Deserializer};
use serde::ser::SerializeStruct;
use serde::de::Error;
use serde_json::{Error as SerdeJsonError, Value, from_value};

use crypto::{Hash, hash};

use super::pair_hash;
use super::super::{StorageValue};
use super::key::ProofListKey;

use self::ListProof::*;

#[derive(Debug, PartialEq, Eq)]
pub enum ListProof<V> {
    Full(Box<ListProof<V>>, Box<ListProof<V>>),
    Left(Box<ListProof<V>>, Option<Hash>),
    Right(Hash, Box<ListProof<V>>),
    Leaf(V),
}

#[derive(Debug)]
pub enum ListProofError {
    UnexpectedLeaf,
    UnexpectedBranch,
    UnmatchedRootHash
}

impl<V: StorageValue> ListProof<V> {
    fn collect<'a>(&'a self, key: ProofListKey, vec: &mut Vec<(u64, &'a V)>)
            -> Result<Hash, ListProofError> {
        if key.height() == 0 {
            return Err(ListProofError::UnexpectedBranch)
        }
        let hash = match *self {
            Full(ref left, ref right) =>
                pair_hash(&left.collect(key.left(), vec)?,
                          &right.collect(key.right(), vec)?),
            Left(ref left, Some(ref right)) =>
                pair_hash(&left.collect(key.left(), vec)?, right),
            Left(ref left, None) =>
                hash(left.collect(key.left(), vec)?.as_ref()),
            Right(ref left, ref right) =>
                pair_hash(left, &right.collect(key.right(), vec)?),
            Leaf(ref value) => {
                if key.height() > 1 {
                    return Err(ListProofError::UnexpectedLeaf)
                }
                vec.push((key.index(), value));
                value.hash()
            }
        };
        Ok(hash)
    }

    pub fn validate<'a>(&'a self, root_hash: Hash, len: u64)
            -> Result<Vec<(u64, &'a V)>, ListProofError> {
        let mut vec = Vec::new();
        let height = len.next_power_of_two().trailing_zeros() as u8 + 1;
        if self.collect(ProofListKey::new(height, 0), &mut vec)? != root_hash {
            return Err(ListProofError::UnmatchedRootHash)
        }
        Ok(vec)
    }
}

impl<V: Serialize> Serialize for ListProof<V> {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error> where S: Serializer {
        use self::ListProof::*;
        let mut state;
        match *self {
            Full(ref left_proof, ref right_proof) => {
                state = ser.serialize_struct("Full", 2)?;
                state.serialize_field("left", left_proof)?;
                state.serialize_field("right", right_proof)?;
            }
            Left(ref left_proof, ref option_hash) => {
                if let Some(ref hash) = *option_hash {
                    state = ser.serialize_struct("Left", 2)?;
                    state.serialize_field("left", left_proof)?;
                    state.serialize_field("right", hash)?;
                } else {
                    state = ser.serialize_struct("Left", 1)?;
                    state.serialize_field("left", left_proof)?;
                }
            }
            Right(ref hash, ref right_proof) => {
                state = ser.serialize_struct("Right", 2)?;
                state.serialize_field("left", hash)?;
                state.serialize_field("right", right_proof)?;
            }
            Leaf(ref val) => {
                state = ser.serialize_struct("Leaf", 1)?;
                state.serialize_field("val", val)?;
            }
        }
        state.end()
    }
}
impl<'a, V> Deserialize<'a> for ListProof<V> where for<'de> V: Deserialize<'de> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'a> {
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
                let left_value: &Value = match map_key_value.get("left") {
                    None => {
                        return Err(D::Error::custom(format!("Invalid json: Key {} not found. \
                                                             Value: {:?}",
                                                            "left",
                                                            json)))
                    }
                    Some(left) => left,
                };
                let right_value: &Value = match map_key_value.get("right") {
                    None => {
                        return Err(D::Error::custom(format!("Invalid json: Key {} not found. \
                                                          Value: {:?}",
                                                            "right",
                                                            json)))
                    }
                    Some(right) => right,
                };
                if right_value.is_string() {
                    let left_proof: ListProof<V> = from_value(left_value.clone()).map_err(|err| {
                            D::Error::custom(format_err_string("ListProof",
                                                               left_value.clone(),
                                                               err))
                        })?;
                    let right_hash: Hash = from_value(right_value.clone()).map_err(|err| {
                            D::Error::custom(format_err_string("Hash", right_value.clone(), err))
                        })?;
                    Left(Box::new(left_proof), Some(right_hash))
                } else if left_value.is_string() {
                    let right_proof: ListProof<V> = from_value(right_value.clone()).map_err(|err| {
                            D::Error::custom(format_err_string("ListProof",
                                                               right_value.clone(),
                                                               err))
                        })?;
                    let left_hash: Hash = from_value(left_value.clone()).map_err(|err| {
                            D::Error::custom(format_err_string("Hash", left_value.clone(), err))
                        })?;
                    Right(left_hash, Box::new(right_proof))
                } else {
                    let left_proof = from_value(left_value.clone()).map_err(|err| {
                            D::Error::custom(format_err_string("ListProof",
                                                               left_value.clone(),
                                                               err))
                        })?;
                    let right_proof = from_value(right_value.clone()).map_err(|err| {
                            D::Error::custom(format_err_string("ListProof",
                                                               right_value.clone(),
                                                               err))
                        })?;
                    Full(Box::new(left_proof), Box::new(right_proof))
                }
            }
            1 => {
                if map_key_value.get("val").is_none() && map_key_value.get("left").is_none() {
                    return Err(D::Error::custom(format!("Invalid json: unknown key met. \
                                                         Expected: {} or {}. json: {:?}",
                                                        "val",
                                                        "left",
                                                        json)));
                }
                if let Some(leaf_value) = map_key_value.get("val") {
                    let val: V = from_value(leaf_value.clone()).map_err(|err| {
                            D::Error::custom(format_err_string("V", leaf_value.clone(), err))
                        })?;
                    Leaf(val)
                } else {
                    // "left" is present
                    let left_value = map_key_value.get("left").unwrap();
                    let left_proof: ListProof<V> = from_value(left_value.clone()).map_err(|err| {
                            D::Error::custom(format_err_string("ListProof",
                                                               left_value.clone(),
                                                               err))
                        })?;
                    Left(Box::new(left_proof), None)
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

// impl<V: fmt::Debug> fmt::Debug for ListProof<V> {
//     fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
//         use self::*;
//         match *self {
//             Full(ref left, ref right) => write!(f, "{{\"left\":{:?},\"right\":{:?}}}", left, right),
//             Left(ref left_proof, ref right_hash) => {
//                 if let Some(ref digest) = *right_hash {
//                     write!(f, "{{\"left\":{:?},\"right\":{:?}}}", left_proof, digest)
//                 } else {
//                     write!(f, "{{\"left\":{:?}}}", left_proof)
//                 }
//             }
//             Right(ref left_hash, ref right) => {
//                 write!(f, "{{\"left\":{:?},\"right\":{:?}}}", left_hash, right)
//             }
//             Leaf(ref val) => write!(f, "{{\"val\":{:?}}}", val),
//         }
//     }
// }
