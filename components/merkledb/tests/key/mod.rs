// Copyright 2020 The Exonum Team
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

use exonum_crypto::{hash, Hash, HASH_SIZE};
use exonum_merkledb::{BinaryKey, ObjectHash};

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Key(pub [u8; HASH_SIZE]);

impl ObjectHash for Key {
    fn object_hash(&self) -> Hash {
        hash(&self.0)
    }
}

impl From<[u8; HASH_SIZE]> for Key {
    fn from(key: [u8; HASH_SIZE]) -> Self {
        Self(key)
    }
}

impl BinaryKey for Key {
    fn size(&self) -> usize {
        HASH_SIZE
    }

    fn write(&self, buffer: &mut [u8]) -> usize {
        buffer.copy_from_slice(&self.0);
        self.0.len()
    }

    fn read(buffer: &[u8]) -> Self::Owned {
        let mut buf = [0; 32];
        buf.copy_from_slice(&buffer);
        Self(buf)
    }
}
