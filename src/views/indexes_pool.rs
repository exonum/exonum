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

use std::{borrow::Cow, cell::Cell, ops::Drop};

use serde::{de::DeserializeOwned, Serialize};
use serde_derive::{Deserialize, Serialize};

use crate::{BinaryKey, BinaryValue};

use super::{index_metadata::IndexType, IndexAccess, IndexAddress, View};

const INDEXES_ROOT: &str = "indexes";
const INDEXES_POOL_NAME: &str = "__INDEXES_POOL__";
const INDEXES_POOL_LEN_NAME: &str = "INDEXES_POOL_LEN";

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct IndexMetadata<V> {
    pub index_type: IndexType,
    pub identifier: u64,
    pub state: V,
}

impl<V> BinaryValue for IndexMetadata<V>
where
    V: Serialize + DeserializeOwned,
{
    fn to_bytes(&self) -> Vec<u8> {
        bincode::serialize(self).unwrap()
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Result<Self, failure::Error> {
        bincode::deserialize(bytes.as_ref()).map_err(From::from)
    }
}

impl<V> IndexMetadata<V> {
    fn index_address(&self) -> IndexAddress {
        IndexAddress::with_root(INDEXES_ROOT).append_bytes(&self.identifier)
    }
}

pub fn index_metadata<T, V>(
    index_access: T,
    index_address: &IndexAddress,
    index_type: IndexType,
) -> (IndexAddress, IndexState<T, V>)
where
    T: IndexAccess,
    V: BinaryValue + Copy + Serialize + DeserializeOwned + Default,
{
    let index_name = if let Some(bytes) = index_address.bytes() {
        concat_keys!(index_address.name(), bytes)
    } else {
        concat_keys!(index_address.name())
    };

    let mut pool = View::new(index_access, IndexAddress::from(INDEXES_POOL_NAME));
    let metadata = if let Some(metadata) = pool.get::<_, IndexMetadata<V>>(&index_name) {
        assert_eq!(
            metadata.index_type, index_type,
            "Index type doesn't match specified"
        );
        metadata
    } else {
        let mut pool_len = View::new(index_access, IndexAddress::from(INDEXES_POOL_LEN_NAME));
        let len: u64 = pool_len.get(&()).unwrap_or_default();

        let metadata = IndexMetadata {
            index_type,
            identifier: len,
            state: V::default(),
        };

        pool_len.put(&(), len + 1);
        pool.put(&index_name, metadata.to_bytes());
        metadata
    };

    let index_address = metadata.index_address();
    let index_state = IndexState::new(index_access, index_name, metadata);
    (index_address, index_state)
}

/// TODO Add documentation. [ECR-2820]
pub struct IndexState<T, V>
where
    V: BinaryValue + Serialize + DeserializeOwned + Default + Copy,
    T: IndexAccess,
{
    index_access: T,
    index_name: Vec<u8>,
    cache: Cell<IndexMetadata<V>>,
}

impl<T, V> IndexState<T, V>
where
    V: BinaryValue + Serialize + DeserializeOwned + Default + Copy,
    T: IndexAccess,
{
    pub fn new(index_access: T, index_name: Vec<u8>, metadata: IndexMetadata<V>) -> Self {
        Self {
            index_access,
            index_name,
            cache: Cell::new(metadata),
        }
    }

    /// TODO Add documentation. [ECR-2820]
    pub fn get(&self) -> V {
        self.cache.get().state
    }

    /// TODO Add documentation. [ECR-2820]
    pub fn set(&mut self, state: V) {
        let mut cache = self.cache.get_mut();
        cache.state = state;
        View::new(self.index_access, IndexAddress::from(INDEXES_POOL_NAME))
            .put(&self.index_name, *cache);
    }
}

impl<T, V> std::fmt::Debug for IndexState<T, V>
where
    T: IndexAccess,
    V: BinaryValue + Serialize + DeserializeOwned + Default + Copy,
{
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("IndexState").finish()
    }
}

// impl<T, V> Drop for IndexState<T, V>
// where
//     V: BinaryValue + Serialize + DeserializeOwned + Default + Copy,
//     T: IndexAccess,
// {
//     fn drop(&mut self) {
//         self.view.put(&self.index_name, self.cache.get());
//     }
// }
