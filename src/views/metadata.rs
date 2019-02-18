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

use std::{borrow::Cow, cell::Cell};

use enum_primitive_derive::Primitive;
use failure::{self, ensure, format_err};
use num_traits::{FromPrimitive, ToPrimitive};
use serde::{de::DeserializeOwned, Serialize};
use serde_derive::{Deserialize, Serialize};

use crate::{BinaryKey, BinaryValue};

use super::{IndexAccess, IndexAddress, View};

/// TODO Add documentation. [ECR-2820]
const INDEXES_POOL_NAME: &str = "__INDEXES_POOL__";
/// TODO Add documentation. [ECR-2820]
const INDEXES_POOL_LEN_NAME: &str = "INDEXES_POOL_LEN";

/// TODO Add documentation. [ECR-2820]
#[derive(Debug, Copy, Clone, PartialEq, Primitive, Serialize, Deserialize)]
pub enum IndexType {
    Map = 1,
    List = 2,
    Entry = 3,
    ValueSet = 4,
    KeySet = 5,
    SparseList = 6,
    ProofList = 7,
    ProofMap = 8,
    Unknown = 255,
}

impl BinaryValue for IndexType {
    fn to_bytes(&self) -> Vec<u8> {
        // `.unwrap()` is safe: IndexType is always in range 1..255
        vec![self.to_u8().unwrap()]
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Result<Self, failure::Error> {
        let bytes = bytes.as_ref();
        ensure!(
            bytes.len() == 1,
            "Wrong buffer size: actual {}, expected 1",
            bytes.len()
        );

        let value = bytes[0];
        Self::from_u8(value).ok_or_else(|| format_err!("Unknown value: {}", value))
    }
}

impl Default for IndexType {
    fn default() -> Self {
        IndexType::Unknown
    }
}

/// TODO Add documentation. [ECR-2820]
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct IndexMetadata<V> {
    index_type: IndexType,
    identifier: u64,
    state: V,
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
        IndexAddress::new().append_bytes(&self.identifier)
    }
}

impl IndexAddress {
    fn index_name(&self) -> Vec<u8> {
        if let Some(bytes) = self.bytes() {
            concat_keys!(self.name(), bytes)
        } else {
            concat_keys!(self.name())
        }
    }
}

/// TODO Add documentation. [ECR-2820]
pub fn index_metadata<T, V>(
    index_access: T,
    index_address: &IndexAddress,
    index_type: IndexType,
) -> (IndexAddress, IndexState<T, V>)
where
    T: IndexAccess,
    V: BinaryValue + Copy + Serialize + DeserializeOwned + Default,
{
    let index_name = index_address.index_name();

    let mut pool = IndexesPool::new(index_access);
    let metadata = if let Some(metadata) = pool.index_metadata(&index_name) {
        assert_eq!(
            metadata.index_type, index_type,
            "Index type doesn't match specified"
        );
        metadata
    } else {
        pool.set_index_metadata(&index_name, index_type)
    };

    let index_address = metadata.index_address();
    let index_state = IndexState::new(index_access, index_name, metadata);
    (index_address, index_state)
}

/// TODO Add documentation. [ECR-2820]
struct IndexesPool<T: IndexAccess>(View<T>);

impl<T: IndexAccess> IndexesPool<T> {
    fn new(index_access: T) -> Self {
        let pool_address = IndexAddress::from(INDEXES_POOL_NAME);
        Self(View::new(index_access, pool_address))
    }

    fn index_metadata<V>(&self, index_name: &[u8]) -> Option<IndexMetadata<V>>
    where
        V: BinaryValue + Serialize + DeserializeOwned + Default + Copy,
    {
        self.0.get(index_name)
    }

    fn set_index_metadata<V>(
        &mut self,
        index_name: &[u8],
        index_type: IndexType,
    ) -> IndexMetadata<V>
    where
        V: BinaryValue + Serialize + DeserializeOwned + Default + Copy,
    {
        let mut pool_len = View::new(
            self.0.index_access,
            IndexAddress::from(INDEXES_POOL_LEN_NAME),
        );
        let len: u64 = pool_len.get(&()).unwrap_or_default();

        let metadata = IndexMetadata {
            index_type,
            identifier: len,
            state: V::default(),
        };

        self.0.put(index_name, metadata);
        pool_len.put(&(), len + 1);
        metadata
    }
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

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use crate::BinaryValue;

    use super::IndexType;

    #[test]
    fn test_index_type_binary_value_correct() {
        let index_type = IndexType::ProofMap;
        let buf = index_type.to_bytes();
        assert_eq!(IndexType::from_bytes(Cow::Owned(buf)).unwrap(), index_type);
    }

    #[test]
    #[should_panic(expected = "Wrong buffer size: actual 2, expected 1")]
    fn test_index_type_binary_value_incorrect_buffer_len() {
        let buf = vec![1, 2];
        IndexType::from_bytes(Cow::Owned(buf)).unwrap();
    }

    #[test]
    #[should_panic(expected = "Unknown value: 127")]
    fn test_index_type_binary_value_incorrect_value() {
        let buf = vec![127];
        IndexType::from_bytes(Cow::Owned(buf)).unwrap();
    }
}
