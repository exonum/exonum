use ::storage::StorageValue;
use ::storage::utils::bytes_to_hex;
use base64::encode;
use ::crypto::{hash, Hash};
use std::fmt;
use serde::{Serialize, Serializer};
const LEFT_DESC: &'static str = "left"; 
const RIGHT_DESC: &'static str = "right"; 
const VAL_DESC: &'static str = "value"; 
#[allow(dead_code)]
pub fn proof_indices_values<V: StorageValue + Clone>(proof: &Proofnode<V>) -> Vec<(usize, &V)> {
    let mut res = Vec::new();
    proof.indices_and_values(0usize, &mut res);
    res
}

pub enum Proofnode<V: StorageValue + Clone> {
    Full(Box<Proofnode<V>>, Box<Proofnode<V>>),
    Left(Box<Proofnode<V>>, Option<Hash>),
    Right(Hash, Box<Proofnode<V>>),
    Leaf(V),
}

fn representation_storage_value<T: StorageValue + Clone>(value: &T) -> String {
    let vec_bytes = value.clone().serialize(); 
    encode(&vec_bytes)
}

impl<V:StorageValue + Clone> Serialize for Proofnode<V> {
    
    fn serialize<S>(&self, ser: &mut S) -> Result<(), S::Error>
        where S: Serializer {
            use self::Proofnode::*; 
            let mut state; 
            match *self {
                Full(ref left_proof, ref right_proof) => {
                    state = ser.serialize_struct("Full", 2)?; 
                    ser.serialize_struct_elt(&mut state, LEFT_DESC, left_proof)?;
                    ser.serialize_struct_elt(&mut state, RIGHT_DESC, right_proof)?; 
                }, 
                Left(ref left_proof, ref option_hash) => {
                    if let Some(ref hash) = *option_hash {
                        state = ser.serialize_struct("Left", 2)?; 
                        let hash_repr = representation_storage_value(hash);
                        ser.serialize_struct_elt(&mut state, LEFT_DESC, left_proof)?;
                        ser.serialize_struct_elt(&mut state, RIGHT_DESC, hash_repr)?;
                    } else {
                        state = ser.serialize_struct("Left", 1)?;
                        ser.serialize_struct_elt(&mut state, LEFT_DESC, left_proof)?;
                    }
                }, 
                Right(ref hash, ref right_proof) => {
                    state = ser.serialize_struct("Right", 2)?; 
                    let hash_repr = representation_storage_value(hash);
                    ser.serialize_struct_elt(&mut state, LEFT_DESC, hash_repr)?;                     
                    ser.serialize_struct_elt(&mut state, RIGHT_DESC, right_proof)?;
                }, 
                Leaf(ref val) => {
                    state = ser.serialize_struct("Leaf", 1)?;
                    let val_repr = representation_storage_value(val);
                    ser.serialize_struct_elt(&mut state, VAL_DESC, val_repr)?; 
                }
            }
            ser.serialize_struct_end(state)
    }
}

impl<V: StorageValue + Clone> Proofnode<V> {
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

impl<V: StorageValue + fmt::Debug + Clone> fmt::Debug for Proofnode<V> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::Proofnode::*; 
        match *self {
            Full(ref left, ref right) => {
                write!(f, "{{ left: {:?}, right: {:?} }}", left, right)
            }
            Left(ref left_proof, ref right_hash) => {
                let hash_repr: String; 
                if let Some(ref digest) = *right_hash {
                    hash_repr = bytes_to_hex(digest); 
                } else {
                    hash_repr = "None".to_string(); 
                }
                write!(f, "{{ left: {:?}, right_hash: {:?} }}", left_proof, hash_repr)
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