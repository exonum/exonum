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
use byteorder::{ByteOrder, LittleEndian};
use hex::FromHex;

use crate::BinaryValue;
use exonum_crypto::{Hash, HashStream};

const EMPTY_LIST_HASH: &str = "c6c0aa07f27493d2f2e5cff56c890a353a20086d6c25ec825128e12ae752b2d9";

#[repr(u8)]
#[derive(Copy, Clone, Debug)]
/// `MerkleDB` hash prefixes.
pub enum HashTag {
    /// Hash prefix of a leaf node of the merkle tree.
    Leaf = 0,
    /// Hash prefix of a branch node of the merkle tree.
    Node = 1,
    /// Hash prefix of the list object.
    ListNode = 2,
}

/// Calculate hash value with the specified prefix.
///
/// Different hashes for leaf and branch nodes are used to secure merkle tree from
/// the pre-image attack.
///
/// See more information [here][1].
///
/// [1]: https://tools.ietf.org/html/rfc6962#section-2.1
impl HashTag {
    ///`HashStream` object with the corresponding hash prefix.
    pub(crate) fn hash_stream(self) -> HashStream {
        HashStream::new().update(&[self as u8])
    }

    /// Convenience method to obtain hashed value of the merkle tree node.
    pub fn hash_node(left_hash: &Hash, right_hash: &Hash) -> Hash {
        HashTag::Node
            .hash_stream()
            .update(left_hash.as_ref())
            .update(right_hash.as_ref())
            .hash()
    }

    /// Convenience method to obtain a hashed value of the merkle tree node with one child.
    pub fn hash_single_node(hash: &Hash) -> Hash {
        HashTag::Node.hash_stream().update(hash.as_ref()).hash()
    }

    /// Convenience method to obtain a hashed value of the merkle tree leaf.
    pub fn hash_leaf(value: &[u8]) -> Hash {
        HashTag::Leaf.hash_stream().update(value).hash()
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
        Hash::from_hex(EMPTY_LIST_HASH).unwrap()
    }

    /// Computes a list hash for the given list of hashes.
    pub fn hash_list(hashes: &[Hash]) -> Hash {
        Self::hash_list_node(hashes.len() as u64, root_hash(hashes))
    }
}

/// Computes a Merkle root hash for a the given list of hashes.
///
/// If `hashes` are empty then `Hash::zero()` value is returned.
fn root_hash(hashes: &[Hash]) -> Hash {
    if hashes.is_empty() {
        return Hash::zero();
    }

    let mut hashes: Vec<Hash> = hashes
        .iter()
        .map(|h| HashTag::hash_leaf(&h.to_bytes()))
        .collect();

    let mut end = hashes.len();
    let mut index = 0;

    while end > 1 {
        let first = hashes[index];

        let result = if index < end - 1 {
            HashTag::hash_node(&first, &hashes[index + 1])
        } else {
            HashTag::hash_single_node(&first)
        };

        hashes[index / 2] = result;

        index += 2;

        if index >= end {
            index = 0;
            end = end / 2 + end % 2;
        }
    }

    hashes[0]
}

/// A common trait for the ability to compute a unique hash.
///
/// Unlike `CryptoHash`, the hash value returned by the `UniqueHash::hash()`
/// method isn't always irreversible. This hash is used, for example, in the
/// storage as a key, as uniqueness is important in this case.
pub trait UniqueHash: BinaryValue {
    /// Returns a hash of the value.
    ///
    /// Hash must be unique, but not necessary cryptographic.
    fn hash(&self) -> Hash {
        exonum_crypto::hash(&self.to_bytes())
    }
}

/// Just returns the origin hash.
impl UniqueHash for Hash {
    fn hash(&self) -> Hash {
        *self
    }
}
