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

use crypto::{hash, CryptoHash, HashStream, Hash, HASH_SIZE};
use storage::StorageValue;

/// Hash prefix of the leaf node of a merkle tree.
pub const LEAF_TAG: u8 = 0x0;
/// Hash prefix of the branch node of a merkle tree.
pub const NODE_TAG: u8 = 0x1;
/// Hash prefix of the list object.
pub const LIST_TAG: u8 = 0x2; // Subject of change in the future.
/// Length of the hash prefix.
pub const PREFIX_SIZE: usize = 1;

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

    fn hash_to(&self, stream: HashStream) -> HashStream;

    fn hash_with_prefix(&self) -> Hash;
}

impl<T: CryptoHash + StorageValue + Clone> UniqueHash for T {
    fn hash(&self) -> Hash {
        CryptoHash::hash(self)
    }

    // To obtain hash with prefix:
    fn hash_to(&self, stream: HashStream) -> HashStream {
        stream.update(&self.clone().into_bytes())
    }

    fn hash_with_prefix(&self) -> Hash {
        hash_with_prefix(LEAF_TAG, &self.clone().into_bytes())
    }
}

/// Convenient method to obtain prefixed value of `StorageValue`.
pub fn hash_leaf<V: StorageValue>(value: V) -> Hash {
    // TODO: fix change value.hash() to value.as_ref()
    HashStream::new()
        .update(&[LEAF_TAG])
        .update(&value.into_bytes())
        .hash()
}

/// Convenient method to obtain prefixed value of `Hash`.
pub fn hash_one(h: &Hash) -> Hash {
    hash_with_prefix(NODE_TAG, h.as_ref())
}

/// Convenient method to obtain prefixed value of concatenation of two hashes.
pub fn hash_pair(h1: &Hash, h2: &Hash) -> Hash {
    let mut hash_bytes = [0u8; HASH_SIZE * 2 + PREFIX_SIZE];
    hash_bytes[0] = NODE_TAG;
    hash_bytes[PREFIX_SIZE..HASH_SIZE + PREFIX_SIZE].copy_from_slice(h1.as_ref());
    hash_bytes[HASH_SIZE + PREFIX_SIZE..HASH_SIZE * 2 + PREFIX_SIZE].copy_from_slice(h2.as_ref());
    hash(&hash_bytes)
}

/// Calculate hash value with specified prefix.
///
/// Different hashes for leaf and branch nodes are used to secure merkle tree from pre-image attack.
/// More information here: https://tools.ietf.org/html/rfc6962#section-2.1
pub fn hash_with_prefix(prefix: u8, value: &[u8]) -> Hash {
    HashStream::new()
        .update(&[prefix])
        .update(value)
        .hash()
}

/// Hash of the list object.
///
/// h = sha-256( LIST_TAG || len as u64 || merkle_root )
pub fn list_hash(len: u64, root: Hash) -> Hash {
    let mut len_bytes = [0; 8];
    LittleEndian::write_u64(&mut len_bytes, len);

    HashStream::new()
        .update(&[LIST_TAG])
        .update(&len_bytes)
        .update(root.as_ref())
        .hash()
}
