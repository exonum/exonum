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

//! Helper module for multisignature transactions.
// TODO move out from helpers [ECR-3222]

use byteorder::{ByteOrder, LittleEndian, WriteBytesExt};
use exonum_merkledb::{
    AccessExt, BinaryKey, BinaryValue, IndexAccessMut, ObjectHash, ProofMapIndex,
};

use std::{
    borrow::Cow,
    collections::BTreeSet,
    fmt,
    io::{Cursor, Write},
};

use crate::crypto::{self, Hash, PublicKey};

/// A set of binary values.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Eq, PartialOrd, Ord)]
pub struct BinarySet<T: Ord>(pub BTreeSet<T>);

impl<T: Ord> BinarySet<T> {
    pub fn new() -> Self {
        Self(BTreeSet::default())
    }
}

impl<T: Ord> Default for BinarySet<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Ord + BinaryValue> BinaryValue for BinarySet<T> {
    fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Cursor::new(Vec::new());
        for value in &self.0 {
            let bytes = value.to_bytes();
            buf.write_u64::<LittleEndian>(bytes.len() as u64).unwrap();
            buf.write_all(&bytes).unwrap();
        }
        buf.into_inner()
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Result<Self, failure::Error> {
        let mut values = BTreeSet::new();

        let mut reader = bytes.as_ref();
        while !reader.is_empty() {
            let bytes_len = LittleEndian::read_u64(reader) as usize;
            reader = &reader[8..];
            let value = T::from_bytes(Cow::Borrowed(&reader[0..bytes_len]))?;
            reader = &reader[bytes_len..];
            values.insert(value);
        }

        Ok(Self(values))
    }
}

impl<T: Ord + BinaryValue> ObjectHash for BinarySet<T> {
    fn object_hash(&self) -> Hash {
        crypto::hash(&self.to_bytes())
    }
}

pub struct ValidatorMultisig<T: AccessExt, V> {
    index: ProofMapIndex<T::Base, V, BinarySet<PublicKey>>,
}

impl<T, V> fmt::Debug for ValidatorMultisig<T, V>
where
    T: AccessExt,
    V: BinaryKey + ObjectHash,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ValidatorMultisig")
            .field("index", &self.index)
            .finish()
    }
}

impl<T, V> ValidatorMultisig<T, V>
where
    T: AccessExt,
    V: BinaryKey + ObjectHash,
{
    pub fn get(index_name: &str, access: T) -> Option<Self> {
        let index = access.proof_map(index_name)?;
        Some(Self { index })
    }

    pub fn confirmed_by(&self, id: &V, author: &PublicKey) -> bool {
        self.index
            .get(id)
            .and_then(|set| {
                if set.0.contains(&author) {
                    Some(())
                } else {
                    None
                }
            })
            .is_some()
    }

    pub fn confirmations(&self, id: &V) -> usize {
        self.index.get(id).map_or(0, |confirms| confirms.0.len())
    }
}

impl<T, V> ValidatorMultisig<T, V>
where
    T: AccessExt,
    T::Base: IndexAccessMut,
    V: BinaryKey + ObjectHash,
{
    pub fn initialize(index_name: &str, access: T) -> Self {
        Self {
            index: access.ensure_proof_map(index_name),
        }
    }

    pub fn confirm(&mut self, id: &V, author: PublicKey) -> usize {
        let mut confirmations = self.index.get(id).unwrap_or_default();
        confirmations.0.insert(author);
        let len = confirmations.0.len();
        self.index.put(id, confirmations);
        len
    }
}

impl<T, V> ObjectHash for ValidatorMultisig<T, V>
where
    T: AccessExt,
    V: BinaryKey + ObjectHash,
{
    fn object_hash(&self) -> Hash {
        self.index.object_hash()
    }
}

#[test]
fn test_validator_values_binary_value() {
    let mut set = BinarySet::default();
    let data = vec![
        b"abacaba1224634abcfdfdfca353".to_vec(),
        b"abacaba1224634abcfdfdfca353ee2224774".to_vec(),
    ];
    set.0.insert(data[1].clone());
    set.0.insert(data[0].clone());
    assert_eq!(set.0.len(), 2);

    let bytes = set.clone().into_bytes();
    let set2 = BinarySet::from_bytes(bytes.into()).unwrap();
    assert_eq!(set, set2);
}
