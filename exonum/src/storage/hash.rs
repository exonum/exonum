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

use crypto::{CryptoHash, Hash, HashStream};
use storage::StorageValue;

#[repr(u8)]
#[derive(Copy, Clone, Debug)]
///MerkleDB hash prefixes.
pub enum HashTag {
    /// Hash prefix of the leaf node of a merkle tree.
    Leaf = 0,
    /// Hash prefix of the branch node of a merkle tree.
    Node = 1,
    /// Hash prefix of the list object.
    List = 2,
}

/// Calculate hash value with specified prefix.
///
/// Different hashes for leaf and branch nodes are used to secure merkle tree from pre-image attack.
/// More information here: https://tools.ietf.org/html/rfc6962#section-2.1
impl HashTag {
    ///`HashStream` object with corresponding hash prefix.
    pub fn hash_stream(self) -> HashStream {
        HashStream::new().update(&[self as u8])
    }

    /// Convenient method to obtain hashed value of merkle tree node.
    pub fn hash_node(left_hash: &Hash, right_hash: &Hash) -> Hash {
        HashTag::Node
            .hash_stream()
            .update(left_hash.as_ref())
            .update(right_hash.as_ref())
            .hash()
    }

    /// Convenient method to obtain hashed value of merkle tree node with one child.
    pub fn hash_single_node(hash: &Hash) -> Hash {
        HashTag::Node.hash_stream().update(hash.as_ref()).hash()
    }

    /// Convenient method to obtain hashed value of merkle tree leaf.
    pub fn hash_leaf<V: StorageValue>(value: V) -> Hash {
        HashTag::Leaf
            .hash_stream()
            .update(&value.into_bytes())
            .hash()
    }

    /// Hash of the list object.
    ///
    /// h = sha-256( HashTag::List || len as u64 || merkle_root )
    pub fn hash_list(len: u64, root: Hash) -> Hash {
        let mut len_bytes = [0; 8];
        LittleEndian::write_u64(&mut len_bytes, len);

        HashStream::new()
            .update(&[HashTag::List as u8])
            .update(&len_bytes)
            .update(root.as_ref())
            .hash()
    }

    /// Default hash of the list object.
    pub fn default_list_hash() -> Hash {
        HashTag::hash_list(0, Hash::default())
    }
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
