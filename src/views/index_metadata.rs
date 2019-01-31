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
use failure::{self, ensure, format_err};
use num_traits::{FromPrimitive, ToPrimitive};
use serde_derive::{Deserialize, Serialize};

use super::{IndexAccess, IndexAddress, View};
use crate::{BinaryValue, Fork};

const INDEX_METADATA_NAME: &str = "__INDEX_METADATA__";
const INDEX_TYPE_NAME: &str = "index_type";
const INDEX_STATE_NAME: &str = "index_state";

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

/// Metadata for each index that currently stored in the merkledb.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndexMetadata {
    /// Type of the specified index.
    pub index_type: IndexType,
}

/// TODO Add documentation. [ECR-2820]
#[derive(Debug, Clone, PartialEq)]
pub struct IndexMetadataAddress(IndexAddress);

impl From<&IndexAddress> for IndexMetadataAddress {
    fn from(address: &IndexAddress) -> Self {
        let address = address.append_name(INDEX_METADATA_NAME);
        IndexMetadataAddress(address)
    }
}

/// TODO Add documentation. [ECR-2820]
struct IndexMetadataView<T: IndexAccess> {
    view: View<T>,
}

impl<T: IndexAccess> IndexMetadataView<T> {
    /// TODO Add documentation. [ECR-2820]
    fn new<I>(index_access: T, address: I) -> Self
    where
        I: Into<IndexMetadataAddress>,
    {
        let address = address.into().0;
        Self {
            view: View::new(index_access, address),
        }
    }

    /// TODO Add documentation. [ECR-2820]
    fn index_metadata(&self) -> Option<IndexMetadata> {
        self.view
            .get(INDEX_TYPE_NAME)
            .map(|index_type| IndexMetadata { index_type })
    }

    /// TODO Add documentation. [ECR-2820]
    fn index_state<V: BinaryValue>(&self) -> Option<V> {
        self.view.get(INDEX_STATE_NAME)
    }

    /// TODO Add documentation. [ECR-2820]
    fn into_inner(self) -> (T, IndexMetadataAddress) {
        (
            self.view.index_access,
            IndexMetadataAddress(self.view.address),
        )
    }
}

impl IndexMetadataView<&Fork> {
    /// TODO Add documentation. [ECR-2820]
    fn set_index_state<V: BinaryValue>(&mut self, state: V) {
        self.view.put(INDEX_STATE_NAME, state);
    }

    /// TODO Add documentation. [ECR-2820]
    fn set_index_metadata(&mut self, metadata: &IndexMetadata) {
        self.view.put(INDEX_TYPE_NAME, metadata.index_type);
    }
}

/// TODO Add documentation. [ECR-2820]
pub fn check_or_create_metadata<T: IndexAccess, I: Into<IndexMetadataAddress>>(
    index_access: T,
    address: I,
    metadata: &IndexMetadata,
) -> IndexMetadataAddress {
    let address = address.into();

    let (index_access, address) = {
        let metadata_view = IndexMetadataView::new(index_access, address);
        if let Some(saved_metadata) = metadata_view.index_metadata() {
            assert_eq!(
                metadata, &saved_metadata,
                "Saved metadata doesn't match specified"
            )
        }
        metadata_view.into_inner()
    };

    // Unsafe method `index_access.fork()` here is safe because we never use fork outside this block.
    #[allow(unsafe_code)]
    unsafe {
        if let Some(fork) = index_access.fork() {
            let mut metadata_view = IndexMetadataView::new(fork, address);
            metadata_view.set_index_metadata(&metadata);
            metadata_view.into_inner().1
        } else {
            address
        }
    }
}

/// TODO Add documentation. [ECR-2820]
pub struct IndexState<T, V>
where
    V: BinaryValue,
    T: IndexAccess,
{
    view: IndexMetadataView<T>,
    state: Cell<Option<V>>,
}

impl<T, V> IndexState<T, V>
where
    V: BinaryValue + Clone + Copy + Default,
    T: IndexAccess,
{
    /// TODO Add documentation. [ECR-2820]
    pub fn new(view: &View<T>) -> Self {
        let view = IndexMetadataView::new(view.index_access.clone(), &view.address);
        Self {
            view,
            state: Cell::new(None),
        }
    }

    /// TODO Add documentation. [ECR-2820]
    pub fn get(&self) -> V {
        if let Some(state) = self.state.get() {
            return state;
        }
        let state = self.view.index_state().unwrap_or_default();
        self.state.set(Some(state));
        state
    }
}

impl<V> IndexState<&Fork, V>
where
    V: BinaryValue + Clone + Copy + Default,
{
    /// TODO Add documentation. [ECR-2820]
    pub fn set(&mut self, state: V) {
        self.state.set(Some(state));
        self.view.set_index_state(state)
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
