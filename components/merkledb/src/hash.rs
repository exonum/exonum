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
use byteorder::{ByteOrder, LittleEndian};
use hex::FromHex;

use exonum_crypto::{Hash, HashStream, HASH_SIZE};

use crate::{proof_map_index::ProofPath, BinaryValue};

const EMPTY_LIST_HASH: &str = "c6c0aa07f27493d2f2e5cff56c890a353a20086d6c25ec825128e12ae752b2d9";
const EMPTY_MAP_HASH: &str = "7324b5c72b51bb5d4c180f1109cfd347b60473882145841c39f3e584576296f9";

#[repr(u8)]
#[derive(Copy, Clone, Debug)]
/// `MerkleDB` hash prefixes.
pub enum HashTag {
    /// Hash prefix of a blob.
    Blob = 0,
    /// Hash prefix of a branch node of the merkle tree.
    ListBranchNode = 1,
    /// Hash prefix of the list object.
    ListNode = 2,
    /// Hash prefix of the map object.
    MapNode = 3,
    /// Hash prefix of the map branch node object.
    MapBranchNode = 4,
}

/// Calculate hash value with the specified prefix.
///
/// In `MerkleDB`, all data is presented as objects. Objects are divided into blobs
/// and collections (lists / maps), which in their turn are divided into hashable and
/// non-hashable. `ProofListIndex` and `ProofMapIndex` relate to hashable collections.
/// For these collections, one can define a hash, which is used to build proof for
/// their contents. In the future, these hashes will be used to build proofs for object
/// hierarchies.
///
/// Different hashes for leaf and branch nodes of the list are used to secure merkle tree
/// from the pre-image attack.
///
/// See more information [here][1].
///
/// [1]: https://tools.ietf.org/html/rfc6962#section-2.1
impl HashTag {
    ///`HashStream` object with the corresponding hash prefix.
    pub(crate) fn hash_stream(self) -> HashStream {
        HashStream::new().update(&[self as u8])
    }

    /// Convenience method to obtain a hashed value of the merkle tree leaf.
    pub fn hash_leaf(value: &[u8]) -> Hash {
        HashTag::Blob.hash_stream().update(value).hash()
    }

    /// Convenience method to obtain hashed value of the merkle tree node.
    pub fn hash_node(left_hash: &Hash, right_hash: &Hash) -> Hash {
        HashTag::ListBranchNode
            .hash_stream()
            .update(left_hash.as_ref())
            .update(right_hash.as_ref())
            .hash()
    }

    /// Convenience method to obtain a hashed value of the merkle tree node with one child.
    pub fn hash_single_node(hash: &Hash) -> Hash {
        HashTag::ListBranchNode
            .hash_stream()
            .update(hash.as_ref())
            .hash()
    }

    /// Hash of the list object.
    ///
    /// ```text
    /// h = sha-256( HashTag::ListNode || len as u64 || merkle_root )
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
    /// h = sha-256( HashTag::ListNode || 0 || Hash::default() )
    /// ```
    pub fn empty_list_hash() -> Hash {
        Hash::from_hex(EMPTY_LIST_HASH).unwrap()
    }

    /// Computes a list hash for the given list of hashes.
    pub fn hash_list(hashes: &[Hash]) -> Hash {
        Self::hash_list_node(hashes.len() as u64, root_hash(hashes))
    }

    /// Hash of the map object.
    ///
    /// ```text
    /// h = sha-256( HashTag::MapNode || merkle_root )
    /// ```
    pub fn hash_map_node(root: Hash) -> Hash {
        HashStream::new()
            .update(&[HashTag::MapNode as u8])
            .update(root.as_ref())
            .hash()
    }

    /// Hash of the map branch node.
    ///
    /// ```text
    /// h = sha-256( HashTag::MapBranchNode || <left_key> || <right_key> || <left_hash> || <right_hash> )
    /// ```
    pub fn hash_map_branch(branch_node: &[u8]) -> Hash {
        HashStream::new()
            .update(&[HashTag::MapBranchNode as u8])
            .update(branch_node)
            .hash()
    }

    /// Hash of the map with single entry.
    ///
    /// ``` text
    /// h = sha-256( HashTag::MapBranchNode || <key> || <child_hash> )
    /// ```
    pub fn hash_single_entry_map(path: &ProofPath, h: &Hash) -> Hash {
        HashStream::new()
            .update(&[HashTag::MapBranchNode as u8])
            .update(path.as_bytes())
            .update(h.as_ref())
            .hash()
    }

    /// Hash of the empty map object.
    ///
    /// Empty map hash:
    /// ```text
    /// sha-256( HashTag::MapNode || Hash::default() )
    /// ```
    pub fn empty_map_hash() -> Hash {
        Hash::from_hex(EMPTY_MAP_HASH).unwrap()
    }
}

/// Computes a Merkle root hash for a the given list of hashes.
///
/// If `hashes` are empty then `Hash::zero()` value is returned.
pub fn root_hash(hashes: &[Hash]) -> Hash {
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
/// Unlike `CryptoHash`, the hash value returned by the `ObjectHash::hash()`
/// method isn't always irreversible. This hash is used, for example, in the
/// storage as a key, as uniqueness is important in this case.
pub trait ObjectHash {
    /// Returns a hash of the value.
    ///
    /// Hash must be unique, but not necessary cryptographic.
    fn object_hash(&self) -> Hash;
}

/// Just returns the origin hash.
impl ObjectHash for Hash {
    fn object_hash(&self) -> Hash {
        *self
    }
}

/// Just returns the origin array.
impl ObjectHash for [u8; HASH_SIZE] {
    fn object_hash(&self) -> Hash {
        Hash::new(*self)
    }
}

#[cfg(test)]
mod tests {
    use exonum_crypto::{Hash, HashStream};

    use super::HashTag;

    #[test]
    fn empty_list_hash() {
        let len_bytes = [0; 8];
        let tag = 2;

        let empty_list_hash = HashStream::new()
            .update(&[tag])
            .update(&len_bytes)
            .update(Hash::default().as_ref())
            .hash();

        assert_eq!(empty_list_hash, HashTag::empty_list_hash());
    }

    #[test]
    fn empty_map_hash() {
        let tag = 3;

        let empty_map_hash = HashStream::new()
            .update(&[tag])
            .update(Hash::default().as_ref())
            .hash();

        assert_eq!(empty_map_hash, HashTag::empty_map_hash());
    }

}
