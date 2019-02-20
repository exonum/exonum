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

use crate::crypto::{CryptoHash, Hash};

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

impl<T: CryptoHash> UniqueHash for T {
    fn hash(&self) -> Hash {
        CryptoHash::hash(self)
    }
}
