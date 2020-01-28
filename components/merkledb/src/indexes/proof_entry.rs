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

//! An implementation of index that may only contain one element.

use std::marker::PhantomData;

use exonum_crypto::Hash;

use crate::{
    access::{Access, AccessError, FromAccess},
    views::{IndexAddress, IndexState, IndexType, RawAccess, RawAccessMut, View, ViewWithMetadata},
    BinaryValue, ObjectHash,
};

/// A hashed index that may only contain one element.
///
/// You can add an element to this index and check whether it exists. A value
/// should implement [`BinaryValue`] and [`ObjectHash`] traits. Unlike [`Entry`],
/// hashed entries are eagerly hashed and may participate in [state aggregation].
///
/// [`BinaryValue`]: ../trait.BinaryValue.html
/// [`ObjectHash`]: ../trait.ObjectHash.html
/// [`Entry`]: struct.Entry.html
/// [state aggregation]: index.html#state-aggregation
#[derive(Debug)]
pub struct ProofEntry<T: RawAccess, V> {
    base: View<T>,
    state: IndexState<T, Hash>,
    _v: PhantomData<V>,
}

impl<T, V> FromAccess<T> for ProofEntry<T::Base, V>
where
    T: Access,
    V: BinaryValue + ObjectHash,
{
    fn from_access(access: T, addr: IndexAddress) -> Result<Self, AccessError> {
        let view = access.get_or_create_view(addr, IndexType::ProofEntry)?;
        Ok(Self::new(view))
    }
}

impl<T, V> ProofEntry<T, V>
where
    T: RawAccess,
    V: BinaryValue,
{
    pub(crate) fn new(view: ViewWithMetadata<T>) -> Self {
        let (base, state) = view.into_parts();
        Self {
            base,
            state,
            _v: PhantomData,
        }
    }

    /// Returns a value of the entry or `None` if does not exist.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{access::CopyAccessExt, TemporaryDB, Database, Entry};
    ///
    /// let db = TemporaryDB::new();
    /// let fork = db.fork();
    /// let mut index = fork.get_proof_entry("name");
    /// assert_eq!(None, index.get());
    ///
    /// index.set(10);
    /// assert_eq!(Some(10), index.get());
    /// ```
    pub fn get(&self) -> Option<V> {
        self.base.get(&())
    }

    /// Returns `true` if a value of the entry exists.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{access::CopyAccessExt, TemporaryDB, Database, Entry};
    ///
    /// let db = TemporaryDB::new();
    /// let fork = db.fork();
    /// let mut index = fork.get_proof_entry("name");
    /// assert!(!index.exists());
    ///
    /// index.set(10);
    /// assert!(index.exists());
    /// ```
    pub fn exists(&self) -> bool {
        self.base.contains(&())
    }
}

impl<T, V> ProofEntry<T, V>
where
    T: RawAccessMut,
    V: BinaryValue + ObjectHash,
{
    /// Changes a value of the entry.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{access::CopyAccessExt, TemporaryDB, Database, Entry};
    ///
    /// let db = TemporaryDB::new();
    /// let fork = db.fork();
    /// let mut index = fork.get_proof_entry("name");
    ///
    /// index.set(10);
    /// assert_eq!(Some(10), index.get());
    /// ```
    pub fn set(&mut self, value: V) {
        self.state.set(value.object_hash());
        self.base.put(&(), value);
    }

    /// Removes a value of the entry.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{access::CopyAccessExt, TemporaryDB, Database, Entry};
    ///
    /// let db = TemporaryDB::new();
    /// let fork = db.fork();
    /// let mut index = fork.get_proof_entry("name");
    ///
    /// index.set(10);
    /// assert_eq!(Some(10), index.get());
    ///
    /// index.remove();
    /// assert_eq!(None, index.get());
    /// ```
    pub fn remove(&mut self) {
        self.state.unset();
        self.base.remove(&());
    }

    /// Takes the value out of the entry, leaving a None in its place.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{access::CopyAccessExt, TemporaryDB, Database, Entry};
    ///
    /// let db = TemporaryDB::new();
    /// let fork = db.fork();
    /// let mut index = fork.get_proof_entry("name");
    ///
    /// index.set(10);
    /// assert_eq!(Some(10), index.get());
    ///
    /// let value = index.take();
    /// assert_eq!(Some(10), value);
    /// assert_eq!(None, index.get());
    /// ```
    pub fn take(&mut self) -> Option<V> {
        let value = self.get();
        if value.is_some() {
            self.remove();
        }
        value
    }

    /// Replaces the value in the entry with the given one, returning the previously stored value.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{access::CopyAccessExt, TemporaryDB, Database, Entry};
    ///
    /// let db = TemporaryDB::new();
    /// let fork = db.fork();
    /// let mut index = fork.get_proof_entry("name");
    ///
    /// index.set(10);
    /// assert_eq!(Some(10), index.get());
    ///
    /// let value = index.swap(20);
    /// assert_eq!(Some(10), value);
    /// assert_eq!(Some(20), index.get());
    /// ```
    pub fn swap(&mut self, value: V) -> Option<V> {
        let previous = self.get();
        self.set(value);
        previous
    }
}

/// `object_hash` is computed as SHA-256 of the entry serialization, or `Hash::zero()` if
/// the entry is not set.
///
/// # Examples
///
/// ```
/// # use exonum_merkledb::{access::CopyAccessExt, TemporaryDB, Database, Entry, ObjectHash};
/// # use exonum_crypto::{self, Hash};
/// let db = TemporaryDB::new();
/// let fork = db.fork();
/// let mut index = fork.get_proof_entry("name");
/// assert_eq!(Hash::default(), index.object_hash());
///
/// let value = 10;
/// index.set(value);
/// assert_eq!(exonum_crypto::hash(&[value]), index.object_hash());
/// ```
impl<T, V> ObjectHash for ProofEntry<T, V>
where
    T: RawAccess,
    V: BinaryValue + ObjectHash,
{
    fn object_hash(&self) -> Hash {
        self.state.get().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{access::CopyAccessExt, Database, TemporaryDB};
    use std::borrow::Cow;

    #[test]
    fn basics() {
        let db = TemporaryDB::new();
        let fork = db.fork();
        {
            let mut entry = fork.get_proof_entry("test");
            assert!(!entry.exists());
            entry.set(25_u64);
            assert!(entry.exists());
            assert_eq!(entry.get(), Some(25));
            assert_eq!(entry.swap(42), Some(25));
        }
        db.merge(fork.into_patch()).unwrap();

        let snapshot = db.snapshot();
        let entry = snapshot.get_proof_entry::<_, u64>("test");
        assert_eq!(entry.get(), Some(42));
        assert_eq!(entry.object_hash(), 42_u64.object_hash());
    }

    #[test]
    fn entry_with_custom_hashing() {
        #[derive(Debug, PartialEq)]
        struct CustomHash(u8);

        impl BinaryValue for CustomHash {
            fn to_bytes(&self) -> Vec<u8> {
                vec![self.0]
            }

            fn from_bytes(bytes: Cow<'_, [u8]>) -> Result<Self, failure::Error> {
                u8::from_bytes(bytes).map(CustomHash)
            }
        }

        impl ObjectHash for CustomHash {
            fn object_hash(&self) -> Hash {
                Hash::new([self.0; exonum_crypto::HASH_SIZE])
            }
        }

        let db = TemporaryDB::new();
        let fork = db.fork();
        {
            let mut entry = fork.get_proof_entry("test");
            entry.set(CustomHash(11));
            assert!(entry.exists());
            assert_eq!(entry.get(), Some(CustomHash(11)));
        }
        db.merge(fork.into_patch()).unwrap();

        let snapshot = db.snapshot();
        let entry = snapshot.get_proof_entry::<_, ()>("test");
        assert_eq!(
            entry.object_hash(),
            Hash::new([11; exonum_crypto::HASH_SIZE])
        );
    }
}
