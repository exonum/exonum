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

use std::{borrow::Cow, fmt};

use serde_derive::{Deserialize, Serialize};
use serde_json::{self, Error as JsonError};

use exonum_crypto;

use crate::{base_index::BaseIndex, BinaryForm, Fork, Snapshot, UniqueHash};

pub const INDEXES_METADATA_TABLE_NAME: &str = "__INDEXES_METADATA__";

// Storage metadata of a current Exonum version.
// Value of this constant is to be changed manually
// upon the introduction of breaking changes to the storage.
const CORE_STORAGE_METADATA: StorageMetadata = StorageMetadata { version: 0 };
const CORE_STORAGE_METADATA_KEY: &str = "__STORAGE_METADATA__";

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
struct IndexMetadata {
    index_type: IndexType,
    is_family: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
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

impl BinaryForm for IndexType {
    fn to_bytes(&self) -> Vec<u8> {
        (*self as u8).to_bytes()
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Result<Self, failure::Error> {
        <u8 as BinaryForm>::from_bytes(bytes).map(Self::from)
    }
}

impl BinaryForm for IndexMetadata {
    fn to_bytes(&self) -> Vec<u8> {
        vec![self.index_type as u8, if self.is_family { 1 } else { 0 }]
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Result<Self, failure::Error> {
        let value = bytes.as_ref();
        let index_type = IndexType::from(value[0]);
        let is_family = value[1] != 0;
        Ok(Self {
            index_type,
            is_family,
        })
    }
}

pub fn assert_index_type(name: &str, index_type: IndexType, is_family: bool, view: &dyn Snapshot) {
    let metadata = BaseIndex::indexes_metadata(view);
    if let Some(value) = metadata.get::<_, IndexMetadata>(name) {
        let stored_type = value.index_type;
        let stored_is_family = value.is_family;
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageMetadata {
    version: u32,
}

impl StorageMetadata {
    pub fn try_serialize(&self) -> Result<Vec<u8>, JsonError> {
        serde_json::to_vec(&self)
    }

    pub fn try_deserialize(serialized: &[u8]) -> Result<Self, JsonError> {
        serde_json::from_slice(serialized)
    }

    pub fn current() -> Self {
        CORE_STORAGE_METADATA
    }

    pub fn write_current(view: &mut Fork) {
        let mut metadata = BaseIndex::indexes_metadata(view);
        metadata.put(&CORE_STORAGE_METADATA_KEY.to_owned(), Self::current());
    }

    pub fn read<T: AsRef<dyn Snapshot>>(view: T) -> Result<Self, super::Error> {
        let metadata = BaseIndex::indexes_metadata(view);
        match metadata.get::<_, Self>(CORE_STORAGE_METADATA_KEY) {
            Some(ref ver) if *ver == CORE_STORAGE_METADATA => Ok(ver.clone()),
            Some(ref ver) => Err(super::Error::new(format!(
                "Unsupported storage version: [{}]. Current storage version: [{}].",
                ver,
                Self::current(),
            ))),
            None => Err(super::Error::new(format!(
                "Storage version is not specified. Current storage version: [{}].",
                Self::current()
            ))),
        }
    }
}

impl BinaryForm for StorageMetadata {
    fn to_bytes(&self) -> Vec<u8> {
        self.try_serialize().unwrap()
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Result<Self, failure::Error> {
        Self::try_deserialize(&bytes).map_err(From::from)
    }
}

impl UniqueHash for StorageMetadata {}

impl fmt::Display for StorageMetadata {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.version)
    }
}

pub fn set_index_type(name: &str, index_type: IndexType, is_family: bool, view: &mut Fork) {
    if name == INDEXES_METADATA_TABLE_NAME || name == CORE_STORAGE_METADATA_KEY {
        panic!("Attempt to access an internal storage infrastructure");
    }
    let mut metadata = BaseIndex::indexes_metadata(view);
    if metadata.get::<_, IndexMetadata>(name).is_none() {
        metadata.put(
            &name.to_owned(),
            IndexMetadata {
                index_type,
                is_family,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{
        IndexMetadata, IndexType, StorageMetadata, CORE_STORAGE_METADATA,
        CORE_STORAGE_METADATA_KEY, INDEXES_METADATA_TABLE_NAME,
    };
    use crate::{base_index::BaseIndex, Database, Fork, MapIndex, ProofMapIndex, TemporaryDB};
    use exonum_crypto::{Hash, PublicKey};

    #[test]
    fn test_index_metadata_roundtrip() {
        use self::IndexType::*;

        let index_types = [
            Entry, KeySet, List, SparseList, Map, ProofList, ProofMap, ValueSet,
        ];
        let is_family = [true, true, false, false, true, false, true, false];
        for (t, f) in index_types.iter().zip(&is_family) {
            let metadata = IndexMetadata {
                index_type: *t,
                is_family: *f,
            };
            assert_eq!(metadata.index_type, *t);
            assert_eq!(metadata.is_family, *f)
        }
    }

    #[test]
    fn test_access_indexes_metadata() {
        let database = TemporaryDB::default();
        let mut fork = database.fork();

        let index: MapIndex<_, String, i32> = MapIndex::new(INDEXES_METADATA_TABLE_NAME, &mut fork);
        assert!(index.get("Test").is_none());
    }

    #[test]
    #[should_panic(expected = "Attempt to access an internal storage infrastructure")]
    fn test_access_indexes_metadata_mut() {
        let database = TemporaryDB::default();
        let mut fork = database.fork();

        let mut index = MapIndex::new(INDEXES_METADATA_TABLE_NAME, &mut fork);
        index.put(&"TestKey".to_string(), 42);
    }

    #[test]
    #[should_panic(expected = "Attempt to access index 'test_index' of type Map, \
                               while said index was initially created with type ProofMap")]
    fn test_invalid_index_type() {
        let database = TemporaryDB::default();
        let mut fork = database.fork();
        {
            let mut index = ProofMapIndex::new("test_index", &mut fork);
            index.put(&PublicKey::zero(), 42);
        }

        let _: MapIndex<_, PublicKey, i32> = MapIndex::new("test_index", &mut fork);
    }

    #[test]
    fn test_valid_index_type() {
        let database = TemporaryDB::default();
        let mut fork = database.fork();
        {
            let mut index = ProofMapIndex::new("test_index", &mut fork);
            index.put(&PublicKey::zero(), 42);
        }

        let _: ProofMapIndex<_, PublicKey, i32> = ProofMapIndex::new("test_index", &mut fork);
    }

    #[test]
    #[should_panic(expected = "Attempt to access index family 'test_index' \
                               while it's an ordinary index")]
    fn test_ordinary_index_as_index_family() {
        let database = TemporaryDB::default();
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
    #[should_panic(expected = "Attempt to access an ordinary index 'test_index' \
                               while it's index family")]
    fn test_index_family_as_ordinary_index() {
        let database = TemporaryDB::default();
        let mut fork = database.fork();
        let index_id: i32 = 42;
        {
            let mut index = MapIndex::new_in_family("test_index", &index_id, &mut fork);
            index.put(&"KEY".to_owned(), 42);
        }

        let _: MapIndex<_, String, i32> = MapIndex::new("test_index", &mut fork);
    }

    #[test]
    fn test_valid_index_type_in_family() {
        let database = TemporaryDB::default();
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
    #[should_panic(expected = "Attempt to access index 'test_index' of type Map, \
                               while said index was initially created with type ProofMap")]
    fn test_multiple_read_before_write() {
        let database = TemporaryDB::default();
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

    #[test]
    fn test_storage_version_write_current() {
        let database = TemporaryDB::default();

        let mut fork = database.fork();
        StorageMetadata::write_current(&mut fork);
        database.merge(fork.into_patch()).unwrap();

        let snap = database.snapshot();

        let core_ver = StorageMetadata::current();

        let read = StorageMetadata::read(snap);
        assert!(read.is_ok());
        assert_eq!(read.unwrap(), core_ver);
    }

    #[test]
    fn test_storage_version_read() {
        let database = TemporaryDB::default();
        {
            let ver = StorageMetadata { version: 1337 };
            let mut fork = database.fork();
            set_storage_version(&mut fork, ver);

            assert!(StorageMetadata::read(fork).is_err());
        }

        {
            let ver = CORE_STORAGE_METADATA;
            let mut fork = database.fork();
            set_storage_version(&mut fork, ver.clone());

            let read = StorageMetadata::read(fork);
            assert!(read.is_ok());
            assert_eq!(read.unwrap(), ver)
        }

        {
            let snap = database.snapshot();

            assert!(StorageMetadata::read(snap).is_err());
        }
    }

    fn set_storage_version(view: &mut Fork, ver: StorageMetadata) {
        let mut metadata = BaseIndex::indexes_metadata(view);
        metadata.put(&CORE_STORAGE_METADATA_KEY.to_owned(), ver);
    }
}
