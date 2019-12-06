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

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use enum_primitive_derive::Primitive;
use exonum_crypto::Hash;
use failure::{self, ensure, format_err};
use num_traits::FromPrimitive;
use serde_derive::{Deserialize, Serialize};

use std::{borrow::Cow, io::Error, mem, num::NonZeroU64};

use super::{
    system_schema::STATE_AGGREGATOR, IndexAddress, RawAccess, RawAccessMut, ResolvedAddress, View,
};
use crate::{
    access::{AccessError, AccessErrorKind},
    validation::check_index_valid_full_name,
    BinaryValue,
};

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
    /// Single entry acting like a Rust `Option`.
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
    /// Merkelized entry.
    ProofEntry = 9,

    /// Unknown index type.
    #[doc(hidden)]
    Unknown = 255,
}

impl IndexType {
    /// Checks if the index of this type is Merkelized.
    fn is_merkelized(self) -> bool {
        match self {
            IndexType::ProofList | IndexType::ProofMap | IndexType::ProofEntry => true,
            _ => false,
        }
    }
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

impl BinaryAttribute for Hash {
    fn size(&self) -> usize {
        exonum_crypto::HASH_SIZE
    }

    fn write(&self, buffer: &mut Vec<u8>) {
        buffer.extend_from_slice(self.as_ref())
    }

    fn read(buffer: &[u8]) -> Result<Self, Error> {
        Hash::from_slice(buffer).ok_or_else(|| {
            Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "Invalid hash length ({}; {} expected)",
                    buffer.len(),
                    exonum_crypto::HASH_SIZE
                ),
            )
        })
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

    fn from_bytes(bytes: Cow<'_, [u8]>) -> Result<Self, failure::Error> {
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
        View::new(
            self.index_access.clone(),
            ResolvedAddress::system(INDEXES_POOL_NAME),
        )
        .put(&self.index_full_name, self.metadata.to_bytes());
    }

    pub fn unset(&mut self) {
        self.metadata.state = None;
        View::new(
            self.index_access.clone(),
            ResolvedAddress::system(INDEXES_POOL_NAME),
        )
        .put(&self.index_full_name, self.metadata.to_bytes());
    }
}

/// Persistent pool used to store indexes metadata in the database.
/// Pool size is used as an identifier of newly created indexes.
pub(super) struct IndexesPool<T: RawAccess>(View<T>);

impl<T: RawAccess> IndexesPool<T> {
    pub(super) fn new(index_access: T) -> Self {
        let view = View::new(index_access, ResolvedAddress::system(INDEXES_POOL_NAME));
        Self(view)
    }

    pub(super) fn len(&self) -> u64 {
        self.0.get(&()).unwrap_or_default()
    }

    fn index_metadata(&self, index_name: &[u8]) -> Option<IndexMetadata> {
        self.0.get(index_name)
    }

    fn set_len(&mut self, len: u64) {
        self.0.put_or_forget(&(), len);
    }

    /// # Return value
    ///
    /// Index metadata and a flag set to `true` if the index is phantom (i.e., is not in the storage
    /// and cannot be persisted because the storage is immutable).
    fn create_index_metadata<V>(
        &mut self,
        index_name: &[u8],
        index_type: IndexType,
    ) -> (IndexMetadata<V>, bool)
    where
        V: BinaryAttribute,
    {
        let len = self.len();
        let metadata = IndexMetadata {
            identifier: len + 1,
            // ^-- Identifier should be non-zero to translate to a correct id in `ResolvedAddress`
            index_type,
            state: None,
        };
        let is_phantom = !self.0.put_or_forget(index_name, metadata.to_bytes());
        self.set_len(len + 1);
        (metadata, is_phantom)
    }
}

/// Obtains `object_hash` for an aggregated index.
pub fn get_object_hash<T: RawAccess>(access: T, addr: ResolvedAddress) -> Hash {
    use crate::{ObjectHash, ProofListIndex, ProofMapIndex};

    let index_full_name = addr.name.as_bytes().to_vec();
    let metadata = IndexesPool::new(access.clone())
        .index_metadata(&index_full_name)
        .unwrap_or_else(|| {
            panic!("Metadata absent for aggregated index {:?}", addr);
        });
    let index_type = metadata.index_type;

    match index_type {
        IndexType::ProofEntry => {
            // Hash is stored directly in the metadata.
            metadata.convert::<Hash>().state.unwrap_or_default()
        }
        IndexType::ProofList | IndexType::ProofMap => {
            let view_with_metadata = ViewWithMetadata {
                view: View::new(access, addr),
                metadata,
                index_full_name,
                is_phantom: false,
            };

            if index_type == IndexType::ProofList {
                // We don't access list elements, so the element type doesn't matter.
                let list = ProofListIndex::<_, ()>::new(view_with_metadata);
                list.object_hash()
            } else {
                // We don't access map elements, so the key / value types don't matter.
                let map = ProofMapIndex::<_, (), ()>::new(view_with_metadata);
                map.object_hash()
            }
        }
        _ => unreachable!(), // other index types are not aggregated
    }
}

/// Wrapper struct to manipulate `IndexMetadata` for an index with provided `index_name`.
#[derive(Debug)]
pub struct ViewWithMetadata<T: RawAccess> {
    view: View<T>,
    metadata: IndexMetadata,
    index_full_name: Vec<u8>,
    is_phantom: bool,
}

impl<T> ViewWithMetadata<T>
where
    T: RawAccess,
{
    /// Gets an index with the specified address and type. Creates an index if it is not present
    /// in the storage.
    pub(crate) fn get_or_create(
        index_access: T,
        index_address: &IndexAddress,
        index_type: IndexType,
    ) -> Result<Self, AccessError> {
        check_index_valid_full_name(index_address)?;
        Self::get_or_create_unchecked(index_access, index_address, index_type)
    }

    /// Gets an index with the specified address and type. Unlike `get_or_create`, this method
    /// does not check if the name of the index is reserved.
    ///
    /// # Safety
    ///
    /// This method should only be used to create system indexes within this crate.
    pub(super) fn get_or_create_unchecked(
        index_access: T,
        index_address: &IndexAddress,
        index_type: IndexType,
    ) -> Result<Self, AccessError> {
        // Actual name.
        let index_name = index_address.name.clone();
        // Full name for internal usage.
        let index_full_name = index_address.fully_qualified_name();

        let mut pool = IndexesPool::new(index_access.clone());
        let mut is_phantom = false;
        let metadata = pool.index_metadata(&index_full_name).unwrap_or_else(|| {
            let (metadata, phantom_flag) = pool.create_index_metadata(&index_full_name, index_type);
            is_phantom = phantom_flag;
            metadata
        });
        let real_index_type = metadata.index_type;
        let addr = ResolvedAddress {
            name: index_name,
            id: NonZeroU64::new(metadata.identifier),
        };

        let is_aggregated = !is_phantom
            && index_type.is_merkelized()
            && index_address.id_in_group.is_none()
            && index_address.name != STATE_AGGREGATOR;

        let mut view = View::new(index_access, addr);
        view.set_or_forget_aggregation(is_aggregated);
        let this = Self {
            view,
            metadata,
            index_full_name,
            is_phantom,
        };

        if real_index_type == index_type {
            Ok(this)
        } else {
            Err(AccessError {
                addr: index_address.clone(),
                kind: AccessErrorKind::WrongIndexType {
                    expected: index_type,
                    actual: real_index_type,
                },
            })
        }
    }

    pub fn index_type(&self) -> IndexType {
        self.metadata.index_type
    }

    pub fn is_phantom(&self) -> bool {
        self.is_phantom
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

impl<T: RawAccess> From<ViewWithMetadata<T>> for View<T> {
    fn from(view_with_metadata: ViewWithMetadata<T>) -> Self {
        view_with_metadata.view
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Database, TemporaryDB};

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

    #[test]
    fn aggregated_indexes_updates() {
        let db = TemporaryDB::new();
        let fork = db.fork();

        // `ListIndex` is not Merkelized.
        let view = ViewWithMetadata::get_or_create(&fork, &"foo".into(), IndexType::List)
            .unwrap()
            .view;
        assert!(!view.changes.is_aggregated());

        // Single `ProofListIndex` is aggregated.
        let view = ViewWithMetadata::get_or_create(&fork, &"bar".into(), IndexType::ProofList)
            .unwrap()
            .view;
        assert!(view.changes.is_aggregated());
        // ...but a `ProofListIndex` in a family isn't.
        let view =
            ViewWithMetadata::get_or_create(&fork, &("baz", &0_u8).into(), IndexType::ProofList)
                .unwrap()
                .view;
        assert!(!view.changes.is_aggregated());
    }
}
