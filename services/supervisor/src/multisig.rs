// Copyright 2020 The Exonum Team
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

use byteorder::{ByteOrder, LittleEndian, WriteBytesExt};
use exonum::{
    crypto::{self, Hash, PublicKey},
    merkledb::{
        access::{Access, RawAccessMut},
        BinaryKey, BinaryValue, Error as MerkledbError, ObjectHash, ProofMapIndex,
    },
};
use exonum_derive::*;

use std::{
    borrow::Cow,
    collections::BTreeSet,
    io::{Cursor, Write},
    mem,
};

/// Wrapper over a `ProofMapIndex` representing a set of values with 0 or more
/// votes for every value.
///
/// Votes are represented as public keys of authors and no verification for
/// ownership is performed within this index.
#[derive(Debug, FromAccess)]
#[from_access(transparent)]
pub struct MultisigIndex<T: Access, V>
where
    V: BinaryKey + ObjectHash,
{
    index: ProofMapIndex<T::Base, V, BinarySet<PublicKey>>,
}

impl<T, V> MultisigIndex<T, V>
where
    T: Access,
    V: BinaryKey + ObjectHash,
{
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

impl<T, V> MultisigIndex<T, V>
where
    T: Access,
    T::Base: RawAccessMut,
    V: BinaryKey + ObjectHash,
{
    pub fn confirm(&mut self, id: &V, author: PublicKey) -> usize {
        let mut confirmations = self.index.get(id).unwrap_or_default();
        confirmations.0.insert(author);
        let len = confirmations.0.len();
        self.index.put(id, confirmations);
        len
    }

    /// Updates the stored confirmations to be an intersection of the set
    /// with current confirmations, and set of the actual validator keys.
    ///
    /// This method is intended to be called before comparing the amount of
    /// confirmations and amount of validators, so confirmations of nodes which
    /// are not validators anymore won't be taken into account.
    ///
    /// Returns the amount of confirmations in the updated set.
    fn intersect(&mut self, id: &V, validator_keys: &BTreeSet<PublicKey>) -> usize {
        let mut confirmations = self.index.get(id).unwrap_or_default();
        confirmations.0 = confirmations
            .0
            .intersection(validator_keys)
            .copied()
            .collect();
        let confirmations_amount = confirmations.0.len();
        self.index.put(id, confirmations);

        confirmations_amount
    }

    /// Calculates the intersection of current confirmations and actual list of
    /// validators.
    ///
    /// Returns `true` if all validators confirmed the item, and `false` otherwise.
    ///
    /// This method updates the list of confirmation, leaving confirmations only from
    /// the actual validators.
    pub fn intersect_with_validators(
        &mut self,
        id: &V,
        validator_keys: impl IntoIterator<Item = PublicKey>,
    ) -> bool {
        let validator_keys: BTreeSet<PublicKey> = validator_keys.into_iter().collect();
        let validators_amount = validator_keys.len();
        self.intersect(id, &validator_keys) == validators_amount
    }
}

impl<T, V> ObjectHash for MultisigIndex<T, V>
where
    T: Access,
    V: BinaryKey + ObjectHash,
{
    fn object_hash(&self) -> Hash {
        self.index.object_hash()
    }
}

/// A set of binary values.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
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

    fn from_bytes(bytes: Cow<'_, [u8]>) -> Result<Self, failure::Error> {
        let mut values = BTreeSet::new();

        // Read the sequence of the (byte size, value bytes) pairs and deserialize them.
        let mut reader = bytes.as_ref();
        while !reader.is_empty() {
            // Verify that buffer size is enough and read the bytes length of the value.
            if reader.len() < mem::size_of::<u64>() {
                return Err(MerkledbError::new("Insufficient buffer size").into());
            }
            let bytes_len = LittleEndian::read_u64(reader) as usize;
            reader = &reader[mem::size_of::<u64>()..];

            // Verify remaining size and read the value.
            if reader.len() < bytes_len {
                return Err(MerkledbError::new("Insufficient buffer size").into());
            }
            let value = T::from_bytes(Cow::Borrowed(&reader[0..bytes_len]))?;
            values.insert(value);
            reader = &reader[bytes_len..];
        }

        Ok(Self(values))
    }
}

impl<T: Ord + BinaryValue> ObjectHash for BinarySet<T> {
    fn object_hash(&self) -> Hash {
        crypto::hash(&self.to_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multisig_values_binary_value() {
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
}
