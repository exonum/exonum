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

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use enum_primitive_derive::Primitive;
use failure::{self, ensure, format_err};
use num_traits::FromPrimitive;
use serde_derive::{Deserialize, Serialize};

use crate::{BinaryKey, BinaryValue};

use super::{IndexAccess, IndexAddress, View};

/// TODO Add documentation. [ECR-2820]
const INDEXES_POOL_NAME: &str = "__INDEXES_POOL__";
/// TODO Add documentation. [ECR-2820]
const INDEXES_POOL_LEN_NAME: &str = "INDEXES_POOL_LEN";

/// TODO Add documentation. [ECR-2820]
#[derive(Debug, Copy, Clone, PartialEq, Primitive, Serialize, Deserialize)]
#[repr(u32)]
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

pub trait BinaryAttribute {
    fn tag() -> Option<u32> {
        None
    }

    fn size(&self) -> usize;

    fn write<W: std::io::Write>(&self, buffer: &mut W);

    fn read<R: std::io::Read>(buffer: &mut R) -> Self;
}

/// No-op implementation.
impl BinaryAttribute for () {
    fn size(&self) -> usize {
        0
    }

    fn write<W: std::io::Write>(&self, _buffer: &mut W) {}

    fn read<R: std::io::Read>(_buffer: &mut R) -> Self {}
}

impl BinaryAttribute for u64 {
    fn size(&self) -> usize {
        std::mem::size_of::<Self>()
    }

    fn write<W: std::io::Write>(&self, buffer: &mut W) {
        buffer.write_u64::<LittleEndian>(*self).unwrap()
    }

    fn read<R: std::io::Read>(buffer: &mut R) -> Self {
        buffer.read_u64::<LittleEndian>().unwrap()
    }
}

impl Default for IndexType {
    fn default() -> Self {
        IndexType::Unknown
    }
}

/// TODO Add documentation. [ECR-2820]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IndexMetadata<V> {
    identifier: u64,
    index_type: IndexType,
    state: V,
}

impl<V> BinaryValue for IndexMetadata<V>
where
    V: BinaryAttribute,
{
    fn to_bytes(&self) -> Vec<u8> {
        let state_len = self.state.size();
        let mut buf = Vec::with_capacity(20 + state_len);

        buf.write_u64::<LittleEndian>(self.identifier).unwrap();
        buf.write_u32::<LittleEndian>(self.index_type as u32)
            .unwrap();
        buf.write_u32::<LittleEndian>(state_len as u32)
            .unwrap();
        self.state.write(&mut buf);
        buf
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Result<Self, failure::Error> {
        let mut bytes = bytes.as_ref();

        let identifier = bytes.read_u64::<LittleEndian>()?;
        let index_type = bytes.read_u32::<LittleEndian>()?;
        let state_len = bytes.read_u32::<LittleEndian>()? as usize;

        ensure!(bytes.len() >= state_len, "Index state is too short");
        let state = V::read(&mut bytes);

        Ok(Self {
            identifier,
            index_type: IndexType::from_u32(index_type)
                .ok_or_else(|| format_err!("Unknown index type: {}", index_type))?,
            state,
        })
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
    V: BinaryAttribute + Copy + Default,
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
        V: BinaryAttribute + Default + Copy,
    {
        self.0.get(index_name)
    }

    fn set_index_metadata<V>(
        &mut self,
        index_name: &[u8],
        index_type: IndexType,
    ) -> IndexMetadata<V>
    where
        V: BinaryAttribute + Default + Copy,
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
    V: BinaryAttribute + Default + Copy,
    T: IndexAccess,
{
    index_access: T,
    index_name: Vec<u8>,
    cache: Cell<IndexMetadata<V>>,
}

impl<T, V> IndexState<T, V>
where
    V: BinaryAttribute + Default + Copy,
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
    V: BinaryAttribute + Default + Copy,
{
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("IndexState").finish()
    }
}

#[cfg(test)]
mod tests {
    use crate::BinaryValue;

    use super::{IndexMetadata, IndexType};

    #[test]
    fn test_index_metadata_binary_value() {
        let metadata = IndexMetadata {
            identifier: 12,
            index_type: IndexType::ProofList,
            state: 0_u64,
        };

        let bytes = metadata.to_bytes();
        assert_eq!(IndexMetadata::from_bytes(bytes.into()).unwrap(), metadata);
    }
}
