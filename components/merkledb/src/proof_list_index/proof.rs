// Copyright 2019 The Exonum Team
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

use serde::{de::Error, ser::SerializeStruct, Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{from_value, Error as SerdeJsonError, Value};

use exonum_crypto::Hash;

use super::{super::BinaryValue, key::ProofListKey, HashTag};

/// An enum that represents a proof of existence/absence for elements in a proof list.
///
/// Proof of absence for an element with the specified index consists of
/// `merkle_root` of `ProofListIndex` and `length` of the list.
///
/// Element with `index` is absent in the list with provided
/// `merkle_root` when two conditions are met:
/// ```text
/// 1. list_hash == sha256( HashTag::List || length || merkle_root )
/// 2. index > length
/// ```
///
/// In case of a range proof this rule applies to the whole range.
#[derive(Debug, Clone, PartialEq, Eq)]
//TODO: serialize/deserialize with serde derive [ECR-3222]
pub enum ProofVariant<V> {
    /// A branch of proof in which both children contain requested elements.
    Full(Box<ProofVariant<V>>, Box<ProofVariant<V>>),
    /// A branch of proof in which only the left child contains requested elements.
    Left(Box<ProofVariant<V>>, Option<Hash>),
    /// A branch of proof in which only the right child contains requested elements.
    Right(Hash, Box<ProofVariant<V>>),
    /// A leaf of the proof with the requested element.
    Leaf(V),
    /// Part of the proof of absence of an element. Full proof is represented by `ListProof`
    /// which also contains length of the list.
    Absent(Hash),
}

/// Wrapper around `ListProofVariant` that represents full proof with the length of the
/// corresponding `ProofList`.
//TODO: deserialize with serde derive [ECR-3222]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ListProof<V> {
    length: u64,
    proof: ProofVariant<V>,
}

impl<V> ListProof<V>
where
    V: BinaryValue,
{
    pub(crate) fn new(length: u64, proof: ProofVariant<V>) -> Self {
        Self { length, proof }
    }

    /// Length of the corresponding ProofList.
    pub fn length(&self) -> u64 {
        self.length
    }

    /// Underlying proof variant.
    pub fn proof_variant(&self) -> &ProofVariant<V> {
        &self.proof
    }

    /// Verifies the correctness of the proof by the trusted list hash.
    /// For more information about list hash see [`HashTag::hash_list_node()`].
    ///
    /// If the proof is valid, a vector with indices and references to elements is returned.
    /// Otherwise, `Err` is returned.
    ///
    /// If the proof is the proof of absence, then empty vector will be returned.
    ///
    /// [`HashTag::hash_list_node()`]: ../enum.HashTag.html#method.hash_list_node
    pub fn validate(&self, expected_list_hash: Hash) -> Result<Vec<(u64, &V)>, ListProofError> {
        let mut vec = Vec::new();
        let height = self.length.next_power_of_two().trailing_zeros() as u8 + 1;
        let merkle_root =
            self.proof
                .collect(ProofListKey::new(height, 0), self.length, &mut vec)?;

        if HashTag::hash_list_node(self.length, merkle_root) != expected_list_hash {
            return Err(ListProofError::UnmatchedRootHash);
        }

        Ok(vec)
    }
}

/// An error that is returned when the list proof is invalid.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Fail)]
pub enum ListProofError {
    /// The proof is too short and does not correspond to the height of the tree.
    #[fail(display = "proof is too short and does not correspond to the height of the tree")]
    UnexpectedLeaf,
    /// The proof is too long and does not correspond to the height of the tree.
    #[fail(display = "proof is too long and does not correspond to the height of the tree")]
    UnexpectedBranch,
    /// The hash of the proof is not equal to the trusted root hash.
    #[fail(display = "hash of the proof is not equal to the trusted root hash")]
    UnmatchedRootHash,
}

impl<V> ProofVariant<V>
where
    V: BinaryValue,
{
    fn collect<'a>(
        &'a self,
        key: ProofListKey,
        length: u64,
        vec: &mut Vec<(u64, &'a V)>,
    ) -> Result<Hash, ListProofError> {
        if key.height() == 0 {
            return Err(ListProofError::UnexpectedBranch);
        }
        let hash = match *self {
            ProofVariant::Full(ref left, ref right) => HashTag::hash_node(
                &left.collect(key.left(), length, vec)?,
                &right.collect(key.right(), length, vec)?,
            ),
            ProofVariant::Left(ref left, Some(ref right)) => {
                HashTag::hash_node(&left.collect(key.left(), length, vec)?, right)
            }
            ProofVariant::Left(ref left, None) => {
                HashTag::hash_single_node(&left.collect(key.left(), length, vec)?)
            }
            ProofVariant::Right(ref left, ref right) => {
                HashTag::hash_node(left, &right.collect(key.right(), length, vec)?)
            }
            ProofVariant::Leaf(ref value) => {
                if key.height() > 1 {
                    return Err(ListProofError::UnexpectedLeaf);
                }
                vec.push((key.index(), value));
                HashTag::hash_leaf(&value.to_bytes())
            }
            ProofVariant::Absent(hash) => hash,
        };
        Ok(hash)
    }
}

impl<V: Serialize> Serialize for ProofVariant<V> {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use self::ProofVariant::*;
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
                    state.serialize_field("right", &hash)?;
                } else {
                    state = ser.serialize_struct("Left", 1)?;
                    state.serialize_field("left", left_proof)?;
                }
            }
            Right(ref hash, ref right_proof) => {
                state = ser.serialize_struct("Right", 2)?;
                state.serialize_field("left", &hash)?;
                state.serialize_field("right", right_proof)?;
            }
            Leaf(ref val) => {
                state = ser.serialize_struct("Leaf", 1)?;
                state.serialize_field("val", val)?;
            }
            ProofVariant::Absent(hash) => {
                state = ser.serialize_struct("Absent", 1)?;
                state.serialize_field("hash", &hash)?;
            }
        }
        state.end()
    }
}

impl<'a, V> Deserialize<'a> for ProofVariant<V>
where
    for<'de> V: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'a>,
    {
        fn format_err_string(type_str: &str, value: &Value, err: &SerdeJsonError) -> String {
            format!(
                "Couldn't deserialize {} from serde_json::Value: {}, error: {}",
                type_str, value, err
            )
        }

        let json: Value = <Value as Deserialize>::deserialize(deserializer)?;
        if !json.is_object() {
            return Err(D::Error::custom(format!(
                "Invalid json: it is expected to be json \
                 Object. json: {:?}",
                json
            )));
        }
        let map_key_value = json.as_object().unwrap();
        let res: Self = match map_key_value.len() {
            2 => {
                let left_value: &Value = match map_key_value.get("left") {
                    None => {
                        return Err(D::Error::custom(format!(
                            "Invalid json: Key {} not found. \
                             Value: {:?}",
                            "left", json
                        )));
                    }
                    Some(left) => left,
                };
                let right_value: &Value = match map_key_value.get("right") {
                    None => {
                        return Err(D::Error::custom(format!(
                            "Invalid json: Key {} not found. \
                             Value: {:?}",
                            "right", json
                        )));
                    }
                    Some(right) => right,
                };
                if right_value.is_string() {
                    let left_proof: Self = from_value(left_value.clone()).map_err(|err| {
                        D::Error::custom(format_err_string("ListProof", left_value, &err))
                    })?;
                    let right_hash: Hash = from_value(right_value.clone()).map_err(|err| {
                        D::Error::custom(format_err_string("Hash", right_value, &err))
                    })?;
                    ProofVariant::Left(Box::new(left_proof), Some(right_hash))
                } else if left_value.is_string() {
                    let right_proof: Self = from_value(right_value.clone()).map_err(|err| {
                        D::Error::custom(format_err_string("ListProof", right_value, &err))
                    })?;
                    let left_hash: Hash = from_value(left_value.clone()).map_err(|err| {
                        D::Error::custom(format_err_string("Hash", left_value, &err))
                    })?;
                    ProofVariant::Right(left_hash, Box::new(right_proof))
                } else {
                    let left_proof = from_value(left_value.clone()).map_err(|err| {
                        D::Error::custom(format_err_string("ListProof", left_value, &err))
                    })?;
                    let right_proof = from_value(right_value.clone()).map_err(|err| {
                        D::Error::custom(format_err_string("ListProof", right_value, &err))
                    })?;
                    ProofVariant::Full(Box::new(left_proof), Box::new(right_proof))
                }
            }
            1 => {
                if map_key_value.get("val").is_none() && map_key_value.get("left").is_none() {
                    return Err(D::Error::custom(format!(
                        "Invalid json: unknown key met. \
                         Expected: {} or {}. json: {:?}",
                        "val", "left", json
                    )));
                }
                if let Some(leaf_value) = map_key_value.get("val") {
                    let val: V = from_value(leaf_value.clone()).map_err(|err| {
                        D::Error::custom(format_err_string("V", leaf_value, &err))
                    })?;
                    ProofVariant::Leaf(val)
                } else {
                    // "left" is present
                    let left_value = map_key_value.get("left").unwrap();
                    let left_proof: Self = from_value(left_value.clone()).map_err(|err| {
                        D::Error::custom(format_err_string("ListProof", left_value, &err))
                    })?;
                    ProofVariant::Left(Box::new(left_proof), None)
                }
            }
            _ => {
                return Err(D::Error::custom(format!(
                    "Invalid json: Number of keys should be \
                     either 1 or 2. json: {:?}",
                    json
                )));
            }
        };
        Ok(res)
    }
}

impl<'a, V> Deserialize<'a> for ListProof<V>
where
    V: BinaryValue,
    for<'de> V: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'a>,
    {
        fn format_err_string(type_str: &str, value: &Value, err: &SerdeJsonError) -> String {
            format!(
                "Couldn't deserialize {} from serde_json::Value: {}, error: {}",
                type_str, value, err
            )
        }

        let json: Value = <Value as Deserialize>::deserialize(deserializer)?;
        if !json.is_object() {
            return Err(D::Error::custom(format!(
                "Invalid json: it is expected to be json \
                 Object. json: {:?}",
                json
            )));
        }
        let map_key_value = json.as_object().unwrap();
        let length;

        if let Some(value) = map_key_value.get("length") {
            length = from_value(value.clone())
                .map_err(|err| D::Error::custom(format_err_string("length", value, &err)))?;
        } else {
            return Err(D::Error::custom(format!(
                "Invalid json: list length is not found. \
                 Value: {:?}",
                json
            )));
        };

        let proof;

        if let Some(value) = map_key_value.get("proof") {
            proof = from_value(value.clone())
                .map_err(|err| D::Error::custom(format_err_string("proof", value, &err)))?;
        } else {
            return Err(D::Error::custom(format!(
                "Invalid json: proof is not found. \
                 Value: {:?}",
                json
            )));
        };

        Ok(Self::new(length, proof))
    }
}
