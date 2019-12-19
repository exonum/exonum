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
use exonum_crypto::{hash, Hash, HashStream};
use failure::Fail;
use hex::FromHex;

use crate::{proof_map_index::ProofPath, BinaryValue};

const EMPTY_LIST_HASH: &str = "c6c0aa07f27493d2f2e5cff56c890a353a20086d6c25ec825128e12ae752b2d9";
const EMPTY_MAP_HASH: &str = "7324b5c72b51bb5d4c180f1109cfd347b60473882145841c39f3e584576296f9";

/// Prefixes for different types of objects stored in the database. These prefixes are necessary
/// to provide domain separation among hashed objects of different types.
///
/// In `MerkleDB`, all data is presented as objects. Objects are divided into blobs
/// and collections (lists / maps), which in their turn are divided into hashable and
/// non-hashable. The hashable collections are `ProofListIndex` and `ProofMapIndex`.
/// For these collections, one can define a single hash which would reflect the entire
/// collection contents. This hash can then be used that a collection contains (or does not contain)
/// certain elements.
///
/// Different hashes for leaf and branch nodes of the list are used to secure merkle tree
/// from the pre-image attack. See more information [here][rfc6962].
///
/// [rfc6962]: https://tools.ietf.org/html/rfc6962#section-2.1
#[derive(Copy, Clone, Debug)]
#[repr(u8)]
pub enum HashTag {
    /// Hash prefix of a blob (i.e., a type implementing [`BinaryValue`], which is stored in the DB
    /// as byte sequence).
    ///
    /// [`BinaryValue`]: trait.BinaryValue.html
    Blob = 0,
    /// Hash prefix of a branch node in a Merkle tree built for
    /// a [Merkelized list](proof_list_index/struct.ProofListIndex.html).
    ListBranchNode = 1,
    /// Hash prefix of a [Merkelized list](proof_list_index/struct.ProofListIndex.html).
    ListNode = 2,
    /// Hash prefix of a [Merkelized map](proof_map_index/struct.ProofMapIndex.html).
    MapNode = 3,
    /// Hash prefix of a branch node in a Merkle Patricia tree built for
    /// a [Merkelized map](proof_map_index/struct.ProofMapIndex.html).
    MapBranchNode = 4,
}

impl HashTag {
    ///`HashStream` object with the corresponding hash prefix.
    pub(crate) fn hash_stream(self) -> HashStream {
        HashStream::new().update(&[self as u8])
    }

    /// Obtains a hashed value of a leaf in a Merkle tree.
    pub fn hash_leaf(value: &[u8]) -> Hash {
        HashTag::Blob.hash_stream().update(value).hash()
    }

    /// Obtains a hashed value of a branch in a Merkle tree.
    pub fn hash_node(left_hash: &Hash, right_hash: &Hash) -> Hash {
        HashTag::ListBranchNode
            .hash_stream()
            .update(left_hash.as_ref())
            .update(right_hash.as_ref())
            .hash()
    }

    /// Obtains a hashed value of a Merkle tree branch with one child.
    pub fn hash_single_node(hash: &Hash) -> Hash {
        HashTag::ListBranchNode
            .hash_stream()
            .update(hash.as_ref())
            .hash()
    }

    /// Obtains hash of a Merkelized list. `len` is the length of the list, and `root` is
    /// the hash of the root node of the Merkle tree corresponding to the list.
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

    /// Hash of an empty Merkelized list.
    ///
    /// ```text
    /// h = sha-256( HashTag::ListNode || 0 || Hash::zero() )
    /// ```
    pub fn empty_list_hash() -> Hash {
        Hash::from_hex(EMPTY_LIST_HASH).unwrap()
    }

    /// Computes the hash for a Merkelized list containing the given values.
    pub fn hash_list<V: BinaryValue + ?Sized>(values: &[V]) -> Hash {
        Self::hash_list_node(values.len() as u64, root_hash(values))
    }

    /// Hash of a Merkelized map with at least 2 entries. `root` is the recursively defined
    /// hash of the root node of the binary Patricia Merkle tree corresponding to the map.
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

    /// Hash of a branch node in a Merkle Patricia tree. `branch_node` is the binary serialization
    /// of the node.
    ///
    /// ```text
    /// h = sha-256(HashTag::MapBranchNode || <branch_node>)
    /// ```
    ///
    /// See [`ProofMapIndex`] for details how branch nodes are serialized.
    ///
    /// [`ProofMapIndex`]: proof_map_index/struct.ProofMapIndex.html#impl-ObjectHash
    pub fn hash_map_branch(branch_node: &[u8]) -> Hash {
        HashStream::new()
            .update(&[HashTag::MapBranchNode as u8])
            .update(branch_node)
            .hash()
    }

    /// Hash of a Merkelized map with a single entry.
    ///
    /// ``` text
    /// h = sha-256( HashTag::MapBranchNode || <path> || <child_hash> )
    /// ```
    pub fn hash_single_entry_map(path: &ProofPath, child_hash: &Hash) -> Hash {
        HashStream::new()
            .update(&[HashTag::MapBranchNode as u8])
            .update(path.as_bytes())
            .update(child_hash.as_ref())
            .hash()
    }

    /// Hash of an empty Merkelized map.
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
pub fn root_hash<V: BinaryValue + ?Sized>(hashes: &[V]) -> Hash {
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
/// The hash value returned by the `object_hash()` method isn't always irreversible.
/// This hash is used, for example, in the storage as a key, as uniqueness is important
/// in this case.
pub trait ObjectHash {
    /// Returns a hash of the value.
    ///
    /// Hash must be unique, but not necessary cryptographic.
    fn object_hash(&self) -> Hash;
}

/// Just returns the original hash.
impl ObjectHash for Hash {
    fn object_hash(&self) -> Hash {
        *self
    }
}

impl ObjectHash for str {
    fn object_hash(&self) -> Hash {
        hash(self.as_bytes())
    }
}

impl ObjectHash for [u8] {
    fn object_hash(&self) -> Hash {
        hash(self)
    }
}

/// Errors that can occur while validating a `ListProof` or `MapProof` against
/// a trusted collection hash.
#[derive(Debug, Fail)]
pub enum ValidationError<E: Fail> {
    /// The hash of the proof is not equal to the trusted root hash.
    #[fail(display = "hash of the proof is not equal to the trusted hash of the list")]
    UnmatchedRootHash,

    /// The proof is malformed.
    #[fail(display = "Malformed proof: {}", _0)]
    Malformed(#[fail(cause)] E),
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
