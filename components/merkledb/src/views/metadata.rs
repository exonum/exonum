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

use std::{borrow::Cow, io::Error, mem};

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use enum_primitive_derive::Primitive;
use failure::{self, ensure, format_err};
use num_traits::FromPrimitive;
use serde_derive::{Deserialize, Serialize};

use super::{IndexAddress, RawAccess, RawAccessMut, View};
use crate::{validation::assert_valid_name, BinaryValue};

/// Name of the column family used to store `IndexesPool`.
const INDEXES_POOL_NAME: &str = "__INDEXES_POOL__";

/// Type of an index supported by Exonum.
///
/// `IndexType` is used for type checking indexes when they are created/accessed.
#[derive(Debug, Copy, Clone, PartialEq, Primitive, Serialize, Deserialize)]
#[repr(u32)]
pub enum IndexType {
    /// Non-merkelized map index.
    Map = 1,
    /// Non-merkelized list index.
    List = 2,
    /// Singleton entry.
    Entry = 3,
    /// Set index with elements stored in a hash table.
    ValueSet = 4,
    /// Set index with elements stored as keys in the underlying KV storage.
    KeySet = 5,
    /// Sparse list index.
    SparseList = 6,
    /// Merkelized list index.
    ProofList = 7,
    /// Merkelized map index.
    ProofMap = 8,

    /// Unknown index type.
    #[doc(hidden)]
    Unknown = 255,
}

/// Index state attribute tag.
const INDEX_STATE_TAG: u32 = 0;

/// A type that can be (de)serialized as a metadata value.
pub trait BinaryAttribute: Sized {
    /// Size of the value.
    fn size(&self) -> usize;
    /// Writes value to specified `buffer`.
    fn write(&self, buffer: &mut Vec<u8>);
    /// Reads value from specified `buffer`.
    fn read(buffer: &[u8]) -> Result<Self, Error>;
}

/// No-op implementation.
impl BinaryAttribute for () {
    fn size(&self) -> usize {
        0
    }

    fn write(&self, _buffer: &mut Vec<u8>) {}

    fn read(_buffer: &[u8]) -> Result<Self, Error> {
        Ok(())
    }
}

impl BinaryAttribute for u64 {
    fn size(&self) -> usize {
        mem::size_of_val(self)
    }

    fn write(&self, buffer: &mut Vec<u8>) {
        buffer.write_u64::<LittleEndian>(*self).unwrap()
    }

    fn read(mut buffer: &[u8]) -> Result<Self, Error> {
        buffer.read_u64::<LittleEndian>()
    }
}

/// Used internally to deserialize generic attribute.
impl BinaryAttribute for Vec<u8> {
    fn size(&self) -> usize {
        self.len()
    }

    fn write(&self, buffer: &mut Vec<u8>) {
        buffer.extend_from_slice(self)
    }

    fn read(buffer: &[u8]) -> Result<Self, Error> {
        Ok(buffer.to_vec())
    }
}

impl Default for IndexType {
    fn default() -> Self {
        IndexType::Unknown
    }
}

/// Metadata associated with each index. Contains `identifier`, `index_type` and `state`.
/// In metadata one can store any arbitrary data serialized as byte array.
///
/// See also `BinaryAttribute`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IndexMetadata<V = Vec<u8>> {
    identifier: u64,
    index_type: IndexType,
    // `state` may be empty for any possible type. `None` option usually represents
    // a "default" value; it is used on index initialization, or after the index
    // calls `IndexState::unset()`. `None` option does not occupy space in the metadata
    // and can therefore be preferable to explicit "default" option.
    state: Option<V>,
}

impl<V> BinaryValue for IndexMetadata<V>
where
    V: BinaryAttribute,
{
    fn to_bytes(&self) -> Vec<u8> {
        let mut capacity = mem::size_of_val(&self.identifier) + mem::size_of_val(&self.index_type);
        if let Some(ref state) = self.state {
            capacity += mem::size_of_val(&INDEX_STATE_TAG) + mem::size_of::<u32>() + state.size();
        }
        let mut buf = Vec::with_capacity(capacity);

        buf.write_u64::<LittleEndian>(self.identifier).unwrap();
        buf.write_u32::<LittleEndian>(self.index_type as u32)
            .unwrap();
        if let Some(ref state) = self.state {
            // Writes index state in TLV (tag, length, value) form.
            buf.write_u32::<LittleEndian>(INDEX_STATE_TAG).unwrap();
            buf.write_u32::<LittleEndian>(state.size() as u32).unwrap();
            state.write(&mut buf);
        }
        buf
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Result<Self, failure::Error> {
        let mut bytes = bytes.as_ref();

        let identifier = bytes.read_u64::<LittleEndian>()?;
        let index_type = bytes.read_u32::<LittleEndian>()?;
        let index_type = IndexType::from_u32(index_type)
            .ok_or_else(|| format_err!("Unknown index type: {}", index_type))?;

        if bytes.is_empty() {
            // There are no tags in the metadata, correspondingly, no index state.
            return Ok(Self {
                identifier,
                index_type,
                state: None,
            });
        }

        // Reads index state in TLV (tag, length, value) form.
        let state_tag = bytes.read_u32::<LittleEndian>()?;
        let state_len = bytes.read_u32::<LittleEndian>()? as usize;

        ensure!(
            state_tag == INDEX_STATE_TAG,
            "Attribute with unknown tag: {}",
            state_tag
        );
        ensure!(bytes.len() >= state_len, "Index state is too short");

        let state_bytes = &bytes[0..state_len];
        Ok(Self {
            identifier,
            index_type,
            state: Some(V::read(state_bytes)?),
        })
    }
}

impl IndexMetadata {
    fn index_address(&self) -> IndexAddress {
        IndexAddress::new().append_bytes(&self.identifier)
    }

    fn convert<V: BinaryAttribute>(self) -> IndexMetadata<V> {
        let index_type = self.index_type;
        IndexMetadata {
            identifier: self.identifier,
            index_type,
            state: self.state.map(|state| {
                V::read(&state).unwrap_or_else(|e| {
                    panic!(
                        "Error while reading state for index with type {:?}: {}. \
                         This can be caused by database corruption",
                        index_type, e
                    );
                })
            }),
        }
    }
}

#[derive(Debug)]
pub struct IndexState<T, V> {
    metadata: IndexMetadata<V>,
    index_access: T,
    index_full_name: Vec<u8>,
}

impl<T, V> IndexState<T, V>
where
    T: RawAccess,
    V: BinaryAttribute + Copy,
{
    pub fn get(&self) -> Option<V> {
        self.metadata.state
    }
}

impl<T, V> IndexState<T, V>
where
    T: RawAccessMut,
    V: BinaryAttribute,
{
    pub fn set(&mut self, state: V) {
        self.metadata.state = Some(state);
        View::new(self.index_access.clone(), INDEXES_POOL_NAME)
            .put(&self.index_full_name, self.metadata.to_bytes());
    }

    pub fn unset(&mut self) {
        self.metadata.state = None;
        View::new(self.index_access.clone(), INDEXES_POOL_NAME)
            .put(&self.index_full_name, self.metadata.to_bytes());
    }
}

/// Persistent pool used to store indexes metadata in the database.
/// Pool size is used as an identifier of newly created indexes.
struct IndexesPool<T: RawAccess>(View<T>);

impl<T: RawAccess> IndexesPool<T> {
    fn new(index_access: T) -> Self {
        Self(View::new(index_access, INDEXES_POOL_NAME))
    }

    fn len(&self) -> u64 {
        self.0.get(&()).unwrap_or_default()
    }

    fn index_metadata(&self, index_name: &[u8]) -> Option<IndexMetadata> {
        self.0.get(index_name)
    }
}

impl<T: RawAccessMut> IndexesPool<T> {
    fn set_len(&mut self, len: u64) {
        self.0.put(&(), len)
    }

    fn create_index_metadata<V>(
        &mut self,
        index_name: &[u8],
        index_type: IndexType,
    ) -> IndexMetadata<V>
    where
        V: BinaryAttribute,
    {
        let len = self.len();
        let metadata = IndexMetadata {
            identifier: len,
            index_type,
            state: None,
        };
        self.0.put(index_name, metadata.to_bytes());
        self.set_len(len + 1);
        metadata
    }
}

/// Wrapper struct to manipulate `IndexMetadata` for an index with provided `index_name`.
#[derive(Debug)]
pub struct ViewWithMetadata<T: RawAccess> {
    view: View<T>,
    metadata: IndexMetadata,
    index_full_name: Vec<u8>,
}

impl<T> ViewWithMetadata<T>
where
    T: RawAccess,
{
    pub(crate) fn get(index_access: T, index_address: &IndexAddress) -> Option<Self> {
        // Actual name.
        let index_name = index_address.name.clone();
        // Full name for internal usage.
        let index_full_name = index_address.fully_qualified_name();

        let pool = IndexesPool::new(index_access.clone());
        let metadata = pool.index_metadata(&index_full_name)?;
        let mut index_address = metadata.index_address();
        // Set index address name, since metadata itself doesn't know it.
        index_address.name = index_name;
        Some(Self {
            view: View::new(index_access, index_address),
            metadata,
            index_full_name,
        })
    }

    pub fn index_type(&self) -> IndexType {
        self.metadata.index_type
    }

    pub(crate) fn assert_type(&self, expected_type: IndexType) {
        assert_eq!(
            self.metadata.index_type, expected_type,
            "Unexpected index type (expected {:?})",
            expected_type
        );
    }

    pub fn into_parts<V>(self) -> (View<T>, IndexState<T, V>)
    where
        V: BinaryAttribute,
    {
        let state = IndexState {
            metadata: self.metadata.convert(),
            index_access: self.view.index_access.clone(),
            index_full_name: self.index_full_name,
        };
        (self.view, state)
    }
}

impl<T> ViewWithMetadata<T>
where
    T: RawAccessMut,
{
    /// # Return value
    ///
    /// Returns `Ok(_)` if the index was newly created or had the expected type, or `Err(_)`
    /// if the index existed and has an unexpected type.
    pub(crate) fn get_or_create(
        index_access: T,
        index_address: &IndexAddress,
        index_type: IndexType,
    ) -> Result<Self, Self> {
        assert_valid_name(index_address.name());

        // Actual name.
        let index_name = index_address.name.clone();
        // Full name for internal usage.
        let index_full_name = index_address.fully_qualified_name();

        let mut pool = IndexesPool::new(index_access.clone());
        let metadata = pool
            .index_metadata(&index_full_name)
            .unwrap_or_else(|| pool.create_index_metadata(&index_full_name, index_type));
        let real_index_type = metadata.index_type;

        let mut index_address = metadata.index_address();
        // Set index address name, since metadata itself doesn't know it.
        index_address.name = index_name;
        let this = Self {
            view: View::new(index_access, index_address),
            metadata,
            index_full_name,
        };
        if real_index_type == index_type {
            Ok(this)
        } else {
            Err(this)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_index_metadata_binary_value() {
        let metadata = IndexMetadata {
            identifier: 12,
            index_type: IndexType::ProofList,
            state: Some(16_u64),
        };

        let bytes = metadata.to_bytes();
        assert_eq!(IndexMetadata::from_bytes(bytes.into()).unwrap(), metadata);

        let metadata = IndexMetadata {
            identifier: 12,
            index_type: IndexType::ProofList,
            state: None::<u64>,
        };

        let bytes = metadata.to_bytes();
        assert_eq!(IndexMetadata::from_bytes(bytes.into()).unwrap(), metadata);
    }

    #[test]
    #[should_panic(expected = "Attribute with unknown tag")]
    fn test_index_metadata_unknown_tag() {
        let metadata = IndexMetadata {
            identifier: 12,
            index_type: IndexType::ProofList,
            state: Some(16_u64),
        };

        let mut bytes = metadata.to_bytes();
        bytes[13] = 1; // Modifies index state tag.
        assert_eq!(IndexMetadata::from_bytes(bytes.into()).unwrap(), metadata);
    }
}
