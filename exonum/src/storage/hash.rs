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
}

impl<T: CryptoHash + StorageValue + Clone> UniqueHash for T {
    fn hash(&self) -> Hash {
        CryptoHash::hash(self)
    }
}
