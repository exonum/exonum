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

#![allow(unsafe_code)]

use std::{borrow::Cow, error::Error};

use crypto::{CryptoHash, Hash};
use encoding::{
    serialize::{json, WriteBufferWrapper}, CheckedOffset, Error as EncodingError, Field, Offset,
};
use storage::{base_index::BaseIndex, Fork, Snapshot, StorageValue};

pub const INDEXES_METADATA_TABLE_NAME: &str = "__INDEXES_METADATA__";

encoding_struct! {
    struct IndexMetadata {
        index_type: IndexType,
        is_family: bool,
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum IndexType {
    Entry,
    KeySet,
    List,
    SparseList,
    Map,
    ProofList,
    ProofMap,
    ValueSet,
}

impl From<u8> for IndexType {
    fn from(num: u8) -> Self {
        use self::IndexType::*;
        match num {
            0 => Entry,
            1 => KeySet,
            2 => List,
            3 => SparseList,
            4 => Map,
            5 => ProofList,
            6 => ProofMap,
            7 => ValueSet,
            invalid => panic!(
                "Unreachable pattern ({:?}) while constructing table type. \
                 Storage data is probably corrupted",
                invalid
            ),
        }
    }
}

impl CryptoHash for IndexType {
    fn hash(&self) -> Hash {
        (*self as u8).hash()
    }
}

impl StorageValue for IndexType {
    fn into_bytes(self) -> Vec<u8> {
        (self as u8).into_bytes()
    }

    fn from_bytes(value: Cow<[u8]>) -> Self {
        <u8 as StorageValue>::from_bytes(value).into()
    }
}

impl<'a> Field<'a> for IndexType {
    unsafe fn read(buffer: &'a [u8], from: Offset, to: Offset) -> Self {
        u8::read(buffer, from, to).into()
    }

    fn write(&self, buffer: &mut Vec<u8>, from: Offset, to: Offset) {
        (*self as u8).write(buffer, from, to)
    }

    fn field_size() -> Offset {
        u8::field_size()
    }

    fn check(
        buffer: &'a [u8],
        from: CheckedOffset,
        to: CheckedOffset,
        latest_segment: CheckedOffset,
    ) -> ::std::result::Result<CheckedOffset, EncodingError> {
        u8::check(buffer, from, to, latest_segment)
    }
}

impl json::ExonumJson for IndexType {
    fn deserialize_field<B: WriteBufferWrapper>(
        value: &json::reexport::Value,
        buffer: &mut B,
        from: Offset,
        to: Offset,
    ) -> Result<(), Box<Error>>
    where
        Self: Sized,
    {
        let v = value.as_u64().ok_or("Can't cast json as u64")? as u8;
        buffer.write(from, to, v);
        Ok(())
    }

    fn serialize_field(&self) -> Result<json::reexport::Value, Box<Error + Send + Sync>> {
        Ok(json::reexport::Value::from(*self as u8))
    }
}

pub fn assert_index_type(name: &str, index_type: IndexType, is_family: bool, view: &Snapshot) {
    let metadata = BaseIndex::indexes_metadata(view);
    if let Some(value) = metadata.get::<_, IndexMetadata>(name) {
        let stored_type = value.index_type();
        let stored_is_family = value.is_family();
        assert_eq!(
            stored_type, index_type,
            "Attempt to access index '{}' of type {:?}, \
             while said index was initially created with type {:?}",
            name, index_type, stored_type
        );
        assert_eq!(
            stored_is_family,
            is_family,
            "Attempt to access {} '{}' while it's {}",
            if is_family {
                "index family"
            } else {
                "an ordinary index"
            },
            name,
            if stored_is_family {
                "index family "
            } else {
                "an ordinary index"
            }
        );
    }
}

pub fn set_index_type(name: &str, index_type: IndexType, is_family: bool, view: &mut Fork) {
    if name == INDEXES_METADATA_TABLE_NAME {
        panic!("Attempt to access an internal storage infrastructure");
    }
    let mut metadata = BaseIndex::indexes_metadata(view);
    if metadata.get::<_, IndexMetadata>(name).is_none() {
        metadata.put(&name.to_owned(), IndexMetadata::new(index_type, is_family));
    }
}

#[cfg(test)]
mod tests {
    use super::{IndexMetadata, IndexType, INDEXES_METADATA_TABLE_NAME};
    use crypto::{Hash, PublicKey};
    use storage::{Database, MapIndex, MemoryDB, ProofMapIndex};

    #[test]
    fn index_metadata_roundtrip() {
        use self::IndexType::*;

        let index_types = [
            Entry, KeySet, List, SparseList, Map, ProofList, ProofMap, ValueSet,
        ];
        let is_family = [true, true, false, false, true, false, true, false];
        for (t, f) in index_types.iter().zip(&is_family) {
            let metadata = IndexMetadata::new(*t, *f);
            assert_eq!(metadata.index_type(), *t);
            assert_eq!(metadata.is_family(), *f)
        }
    }

    #[test]
    fn access_indexes_metadata() {
        let database = MemoryDB::new();
        let mut fork = database.fork();

        let index: MapIndex<_, String, i32> = MapIndex::new(INDEXES_METADATA_TABLE_NAME, &mut fork);
        assert!(index.get("Test").is_none());
    }

    #[test]
    #[should_panic(expected = "Attempt to access an internal storage infrastructure")]
    fn access_indexes_metadata_mut() {
        let database = MemoryDB::new();
        let mut fork = database.fork();

        let mut index = MapIndex::new(INDEXES_METADATA_TABLE_NAME, &mut fork);
        index.put(&"TestKey".to_string(), 42);
    }

    #[test]
    #[should_panic(
        expected = "Attempt to access index 'test_index' of type Map, \
                    while said index was initially created with type ProofMap"
    )]
    fn invalid_index_type() {
        let database = MemoryDB::new();
        let mut fork = database.fork();
        {
            let mut index = ProofMapIndex::new("test_index", &mut fork);
            index.put(&PublicKey::zero(), 42);
        }

        let _: MapIndex<_, PublicKey, i32> = MapIndex::new("test_index", &mut fork);
    }

    #[test]
    fn valid_index_type() {
        let database = MemoryDB::new();
        let mut fork = database.fork();
        {
            let mut index = ProofMapIndex::new("test_index", &mut fork);
            index.put(&PublicKey::zero(), 42);
        }

        let _: ProofMapIndex<_, PublicKey, i32> = ProofMapIndex::new("test_index", &mut fork);
    }

    #[test]
    #[should_panic(
        expected = "Attempt to access index family 'test_index' \
                    while it's an ordinary index"
    )]
    fn ordinary_index_as_index_family() {
        let database = MemoryDB::new();
        let mut fork = database.fork();
        let index_id: i32 = 42;
        {
            let mut index = MapIndex::new("test_index", &mut fork);
            index.put(&"KEY".to_owned(), 42);
        }

        let _: MapIndex<_, String, i32> =
            MapIndex::new_in_family("test_index", &index_id, &mut fork);
    }

    #[test]
    #[should_panic(
        expected = "Attempt to access an ordinary index 'test_index' \
                    while it's index family"
    )]
    fn index_family_as_ordinary_index() {
        let database = MemoryDB::new();
        let mut fork = database.fork();
        let index_id: i32 = 42;
        {
            let mut index = MapIndex::new_in_family("test_index", &index_id, &mut fork);
            index.put(&"KEY".to_owned(), 42);
        }

        let _: MapIndex<_, String, i32> = MapIndex::new("test_index", &mut fork);
    }

    #[test]
    fn valid_index_type_in_family() {
        let database = MemoryDB::new();
        let mut fork = database.fork();
        let index_id: i32 = 42;
        {
            let mut index = ProofMapIndex::new_in_family("test_index", &index_id, &mut fork);
            index.put(&Hash::zero(), 42);
        }

        let _: ProofMapIndex<_, Hash, i32> =
            ProofMapIndex::new_in_family("test_index", &index_id, &mut fork);
    }

    #[test]
    #[should_panic(
        expected = "Attempt to access index 'test_index' of type Map, \
                    while said index was initially created with type ProofMap"
    )]
    fn multiple_read_before_write() {
        let database = MemoryDB::new();
        let mut fork = database.fork();

        // Type is unlocked, can read with any
        {
            let index: MapIndex<_, Hash, i32> = MapIndex::new("test_index", &mut fork);
            assert!(index.get(&Hash::zero()).is_none());
        }

        {
            let index: ProofMapIndex<_, Hash, i32> = ProofMapIndex::new("test_index", &mut fork);
            assert!(index.get(&Hash::zero()).is_none());
        }

        // Lock the type
        {
            let mut index = ProofMapIndex::new("test_index", &mut fork);
            index.put(&Hash::zero(), 42);
        }
        {
            let index: ProofMapIndex<_, Hash, i32> = ProofMapIndex::new("test_index", &mut fork);
            assert_eq!(index.get(&Hash::zero()), Some(42));
        }

        // Make sure we're unable to read with different type now
        let mut index = MapIndex::new("test_index", &mut fork);
        index.put(&Hash::zero(), 43);
    }
}
