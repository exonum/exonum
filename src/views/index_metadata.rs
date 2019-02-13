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

use std::{borrow::Cow, cell::Cell, fmt};

use enum_primitive_derive::Primitive;
use failure::{self, bail, ensure, format_err};
use num_traits::{FromPrimitive, ToPrimitive};
use serde_derive::{Deserialize, Serialize};

use super::{IndexAccess, IndexAddress, View};
use crate::{BinaryValue, Fork};

/// TODO Add documentation. [ECR-2820]
const INDEX_METADATA_NAME: &str = "__INDEX_METADATA__";
/// TODO Add documentation. [ECR-2820]
const INDEX_STATE_NAME: &str = "__INDEX_STATE__";
/// TODO Add documentation. [ECR-2820]
const INDEX_TYPE_NAME: &str = "metadata";

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

/// Metadata for each index that currently stored in the merkledb.
#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndexMetadata {
    /// Type of the specified index.
    pub index_type: IndexType,
    /// TODO Add documentation. [ECR-2820]
    pub has_parent: bool,
}

impl BinaryValue for IndexMetadata {
    fn to_bytes(&self) -> Vec<u8> {
        // `.unwrap()` is safe: IndexType is always in range 1..255
        vec![
            self.index_type.to_u8().unwrap(),
            if self.has_parent { 1 } else { 0 },
        ]
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Result<Self, failure::Error> {
        let bytes = bytes.as_ref();
        ensure!(
            bytes.len() == 2,
            "Wrong buffer size: actual {}, expected 2",
            bytes.len()
        );

        let index_type = IndexType::from_u8(bytes[0])
            .ok_or_else(|| format_err!("Unknown index_type: {}", bytes[0]))?;
        let has_parent = match bytes[1] {
            0 => false,
            1 => true,
            i => bail!("Unknown has_parent: {}", i),
        };

        Ok(Self {
            index_type,
            has_parent,
        })
    }
}

/// TODO Add documentation. [ECR-2820]
struct IndexMetadataView<T: IndexAccess> {
    view: View<T>,
}

impl<T: IndexAccess> IndexMetadataView<T> {
    /// TODO Add documentation. [ECR-2820]
    fn new(index_access: T, address: &IndexAddress) -> Self {
        let metadata_address = IndexAddress {
            // Because `concat` is faster than `format!("...")` in all cases.
            name: [address.name(), ".", INDEX_METADATA_NAME].concat(),
            bytes: None,
        };
        Self {
            view: View::new(index_access, metadata_address),
        }
    }

    /// TODO Add documentation. [ECR-2820]
    fn index_metadata(&self) -> Option<IndexMetadata> {
        self.view.get(INDEX_TYPE_NAME)
    }

    /// TODO Add documentation. [ECR-2820]
    fn set_index_metadata(&mut self, metadata: IndexMetadata) {
        self.view.put(INDEX_TYPE_NAME, metadata);
    }
}

impl IndexMetadataView<&Fork> {}

/// TODO Add documentation. [ECR-2820]
pub fn check_or_create_metadata<T>(index_access: T, address: &IndexAddress, metadata: IndexMetadata)
where
    T: IndexAccess,
{
    let mut view = IndexMetadataView::new(index_access, address);
    if let Some(saved_metadata) = view.index_metadata() {
        assert_eq!(
            metadata, saved_metadata,
            "Saved metadata doesn't match specified"
        );
    } else {
        view.set_index_metadata(metadata);
    }
}

/// TODO Add documentation. [ECR-2820]
pub struct IndexState<T, V>
where
    V: BinaryValue,
    T: IndexAccess,
{
    view: View<T>,
    cache: Cell<Option<V>>,
}

impl<T, V> IndexState<T, V>
where
    V: BinaryValue + Copy + Default,
    T: IndexAccess,
{
    /// TODO Add documentation. [ECR-2820]
    pub fn from_view(view: &View<T>) -> Self {
        let index_state_address = IndexAddress {
            // Because `concat` is faster than `format!("...")` in all cases.
            name: [&view.address.name, ".", INDEX_STATE_NAME].concat(),
            bytes: view.address.bytes.clone(),
        };

        Self {
            view: View::new(view.index_access, index_state_address),
            cache: Cell::new(None),
        }
    }

    /// TODO Add documentation. [ECR-2820]
    pub fn get(&self) -> V {
        if let Some(state) = self.cache.get() {
            return state;
        }
        let state = self.view.get(&()).unwrap_or_default();
        self.cache.set(Some(state));
        state
    }

    /// TODO Add documentation. [ECR-2820]
    pub fn set(&mut self, state: V) {
        self.cache.set(Some(state));
        self.view.put(&(), state)
    }
}

impl<T, V> fmt::Debug for IndexState<T, V>
where
    T: IndexAccess,
    V: BinaryValue,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "IndexState(..)")
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;
    use crate::BinaryValue;
    use super::{IndexType};

    use crate::{
        views::{IndexAddress, View},
        BinaryValue, Database, TemporaryDB,
    };

    use super::{
        IndexMetadata, IndexMetadataView, IndexState, IndexType, INDEX_METADATA_NAME,
        INDEX_STATE_NAME,
    };

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

    #[test]
    fn test_index_metadata_binary_value_correct() {
        let index_metadata = IndexMetadata {
            index_type: IndexType::ProofMap,
            has_parent: true,
        };
        let buf = index_metadata.to_bytes();
        assert_eq!(
            IndexMetadata::from_bytes(Cow::Owned(buf)).unwrap(),
            index_metadata
        );
    }

    #[test]
    #[should_panic(expected = "Wrong buffer size: actual 3, expected 2")]
    fn test_index_metadata_binary_value_incorrect_buffer_len() {
        let buf = vec![1, 2, 3];
        IndexMetadata::from_bytes(Cow::Owned(buf)).unwrap();
    }

    #[test]
    #[should_panic(expected = "Unknown has_parent: 3")]
    fn test_index_metadata_binary_value_incorrect_value() {
        let buf = vec![1, 3];
        IndexMetadata::from_bytes(Cow::Owned(buf)).unwrap();
    }

    #[test]
    fn test_index_metadata_address() {
        let db = TemporaryDB::default();
        let fork = &db.fork();

        let address = IndexAddress::default()
            .append_name("foo")
            .append_bytes("bar");
        let metadata_view = IndexMetadataView::new(fork, &address);

        assert_eq!(
            metadata_view.view.address,
            IndexAddress::default()
                .append_name("foo")
                .append_name(INDEX_METADATA_NAME)
        );
    }

    #[test]
    fn test_index_state_address() {
        let db = TemporaryDB::default();
        let fork = &db.fork();

        let address = IndexAddress::default()
            .append_name("foo")
            .append_bytes("bar");;
        let view = View::new(fork, address);
        let index_state: IndexState<_, ()> = IndexState::from_view(&view);

        assert_eq!(
            index_state.view.address,
            IndexAddress::default()
                .append_name("foo")
                .append_name(INDEX_STATE_NAME)
                .append_bytes("bar")
        );
    }
}
