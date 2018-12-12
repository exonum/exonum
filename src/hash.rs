// Copyright 2018 The Exonum Team
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
use byteorder::ByteOrder;
use bytes::LittleEndian;
use hex::FromHex;

use crate::StorageValue;
use exonum_crypto::{CryptoHash, Hash, HashStream};

#[repr(u8)]
#[derive(Copy, Clone, Debug)]
///MerkleDB hash prefixes.
pub enum HashTag {
    /// Hash prefix of the leaf node of a merkle tree.
    Leaf = 0,
    /// Hash prefix of the branch node of a merkle tree.
    Node = 1,
    /// Hash prefix of the list object.
    ListNode = 2,
}

/// Calculate hash value with specified prefix.
///
/// Different hashes for leaf and branch nodes are used to secure merkle tree from pre-image attack.
/// More information [here][1].
///
/// [1]: https://tools.ietf.org/html/rfc6962#section-2.1
impl HashTag {
    ///`HashStream` object with corresponding hash prefix.
    pub(crate) fn hash_stream(self) -> HashStream {
        HashStream::new().update(&[self as u8])
    }

    /// Convenience method to obtain hashed value of merkle tree node.
    pub fn hash_node(left_hash: &Hash, right_hash: &Hash) -> Hash {
        HashTag::Node
            .hash_stream()
            .update(left_hash.as_ref())
            .update(right_hash.as_ref())
            .hash()
    }

    /// Convenience method to obtain hashed value of merkle tree node with one child.
    pub fn hash_single_node(hash: &Hash) -> Hash {
        HashTag::Node.hash_stream().update(hash.as_ref()).hash()
    }

    /// Convenience method to obtain hashed value of merkle tree leaf.
    pub fn hash_leaf<V: StorageValue>(value: V) -> Hash {
        HashTag::Leaf
            .hash_stream()
            .update(&value.into_bytes())
            .hash()
    }

    /// Hash of the list object.
    ///
    /// ```text
    /// h = sha-256( HashTag::List || len as u64 || merkle_root )
    /// ```
    pub fn hash_list_node(len: u64, root: Hash) -> Hash {
        let mut len_bytes = [0; 8];
        LittleEndian::write_u64(&mut len_bytes, len);

        HashStream::new()
            .update(&[HashTag::ListNode as u8])
            .update(&len_bytes)
            .update(root.as_ref())
            .hash()
    }

    /// Hash of the empty list object.
    ///
    /// Empty list hash:
    /// ```text
    /// h = sha-256( HashTag::List || 0 || Hash::default() )
    /// ```
    pub fn empty_list_hash() -> Hash {
        Hash::from_hex("c6c0aa07f27493d2f2e5cff56c890a353a20086d6c25ec825128e12ae752b2d9").unwrap()
    }

    /// Computes list hash for a given list of hashes.
    pub fn hash_list(hashes: &[Hash]) -> Hash {
        Self::hash_list_node(hashes.len() as u64, root_hash(hashes))
    }
}

/// Computes Merkle root hash for a given list of hashes.
///
/// If `hashes` are empty then `Hash::zero()` value is returned.
fn root_hash(hashes: &[Hash]) -> Hash {
    match hashes.len() {
        0 => Hash::zero(),
        1 => HashTag::hash_leaf(hashes[0]),
        _ => {
            let hashes: Vec<Hash> = hashes.iter().map(|h| HashTag::hash_leaf(*h)).collect();

            let mut current_hashes = combine_hash_list(&hashes);

            while current_hashes.len() > 1 {
                current_hashes = combine_hash_list(&current_hashes);
            }
            current_hashes[0]
        }
    }
}

fn combine_hash_list(hashes: &[Hash]) -> Vec<Hash> {
    hashes
        .chunks(2)
        .map(|pair| match pair {
            [first, second] => HashTag::hash_node(first, second),
            [single] => HashTag::hash_single_node(single),
            _ => unreachable!(),
        })
        .collect()
}

/// A common trait for the ability to compute a unique hash.
///
/// Unlike `CryptoHash`, the hash value returned by the `UniqueHash::hash()`
/// method isn't always irreversible. This hash is used, for example, in the
/// storage as a key, as uniqueness is important in this case.
pub trait UniqueHash {
    /// Returns a hash of the value.
    ///
    /// Hash must be unique, but not necessary cryptographic.
    fn hash(&self) -> Hash;
}

impl<T: CryptoHash + StorageValue + Clone> UniqueHash for T {
    fn hash(&self) -> Hash {
        CryptoHash::hash(self)
    }
}
