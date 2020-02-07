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

//! Access generalizations, mainly useful for bindings.
//!
//! This module provides:
//!
//! - [`GenericRawAccess`], an enumeration of all available types of raw accesses (e.g., `Snapshot`
//!   or `Fork`)
//! - [`GenericAccess`], an enumeration of all high-level access types (e.g., `Prefixed`
//!   or `Migration`)
//! - [`ErasedAccess`], which combines the previous two types and thus is the most abstract kind
//!   of access to the database.
//!
//! [`GenericRawAccess`]: enum.GenericRawAccess.html
//! [`GenericAccess`]: enum.GenericAccess.html
//! [`ErasedAccess`]: type.ErasedAccess.html
//!
//! # Examples
//!
//! Basic usage of `ErasedAccess`:
//!
//! ```
//! use exonum_merkledb::{
//!     access::{AccessExt, Prefixed}, migration::Migration, Database, TemporaryDB,
//! };
//! use exonum_merkledb::generic::{ErasedAccess, IntoErased};
//!
//! fn manipulate_db(access: &ErasedAccess<'_>) {
//!     assert!(access.is_mutable());
//!     let mut list = access.get_list::<_, u32>("list");
//!     list.extend(vec![1, 2, 3]);
//!     access.get_proof_entry("entry").set("!".to_owned());
//! }
//!
//! fn check_db(access: &ErasedAccess<'_>) {
//!     assert!(!access.is_mutable());
//!     let list = access.get_list::<_, u32>("list");
//!     assert_eq!(list.len(), 3);
//!     assert_eq!(list.iter().collect::<Vec<_>>(), vec![1, 2, 3]);
//!     let entry = access.get_proof_entry::<_, String>("entry");
//!     assert_eq!(entry.get().unwrap(), "!");
//! }
//!
//! let db = TemporaryDB::new();
//! let fork = db.fork();
//! // Create a `Prefixed` access and use `IntoErased` trait to convert it
//! // to the most generic access.
//! {
//!     let erased = Prefixed::new("ns", &fork).into_erased();
//!     manipulate_db(&erased);
//! }
//! // The same method may be applied to other access kinds, e.g., `Migration`s.
//! {
//!     let erased = Migration::new("other-ns", &fork).into_erased();
//!     manipulate_db(&erased);
//! }
//! db.merge(fork.into_patch()).unwrap();
//!
//! let snapshot = db.snapshot();
//! let erased = Prefixed::new("ns", snapshot.as_ref()).into_erased();
//! check_db(&erased);
//! let erased = Migration::new("other-ns", snapshot.as_ref()).into_erased();
//! check_db(&erased);
//! ```
//!
//! Use of `GenericRawAccess` with owned accesses:
//!
//! ```
//! use exonum_merkledb::{access::AccessExt, Database, TemporaryDB};
//! use exonum_merkledb::generic::GenericRawAccess;
//! use std::rc::Rc;
//!
//! let db = TemporaryDB::new();
//! let fork = db.fork();
//! let access = GenericRawAccess::from(fork); // Consumes `fork`!
//! access.get_proof_map("list").put("foo", "bar".to_owned());
//! // Get `Fork` back from the access. The caller should ensure
//! // that `access` is not used elsewhere at this point, e.g.,
//! // by instantiated indexes.
//! let fork = match access {
//!     GenericRawAccess::OwnedFork(fork) => Rc::try_unwrap(fork).unwrap(),
//!     _ => unreachable!(),
//! };
//! db.merge(fork.into_patch()).unwrap();
//! ```

use std::rc::Rc;

use crate::{
    access::{Access, AccessError, AsReadonly, Prefixed},
    db::{ChangesMut, ChangesRef, ViewChanges},
    migration::{Migration, Scratchpad},
    views::{ChangeSet, GroupKeys, IndexMetadata, RawAccess, RawAccessMut, ViewWithMetadata},
    BinaryKey, Fork, IndexAddress, IndexType, OwnedReadonlyFork, ReadonlyFork, ResolvedAddress,
    Snapshot,
};

/// Container for an arbitrary raw access. For `Fork`s and `Snapshot`s, this type provides
/// both owned and borrowed variants.
///
/// `GenericRawAccess` implements [`RawAccess`] and [`RawAccessMut`] traits. The latter
/// means that the mutable methods on indexes will panic in the run time if an immutable access
/// (such as a `Snapshot`) is used as the base. The caller is advised to check
/// mutability in advance with the help of [`is_mutable()`].
///
/// This type is not intended to be exhaustively matched. It can be extended in the future
/// without breaking the semver compatibility.
///
/// [`RawAccess`]: ../access/trait.RawAccess.html
/// [`RawAccessMut`]: ../access/trait.RawAccessMut.html
/// [`is_mutable()`]: #method.is_mutable
#[derive(Debug, Clone)]
pub enum GenericRawAccess<'a> {
    /// Borrowed snapshot.
    Snapshot(&'a dyn Snapshot),
    /// Owned snapshot.
    OwnedSnapshot(Rc<dyn Snapshot>),
    /// Borrowed fork.
    Fork(&'a Fork),
    /// Owned fork.
    OwnedFork(Rc<Fork>),
    /// Readonly fork.
    ReadonlyFork(ReadonlyFork<'a>),
    /// Owned readonly fork.
    OwnedReadonlyFork(OwnedReadonlyFork),

    /// Never actually generated.
    #[doc(hidden)]
    __NonExhaustive,
}

impl GenericRawAccess<'_> {
    /// Checks if the underlying access is mutable.
    pub fn is_mutable(&self) -> bool {
        match self {
            GenericRawAccess::Fork(_) | GenericRawAccess::OwnedFork(_) => true,
            _ => false,
        }
    }
}

impl<'a> From<&'a dyn Snapshot> for GenericRawAccess<'a> {
    fn from(snapshot: &'a dyn Snapshot) -> Self {
        GenericRawAccess::Snapshot(snapshot)
    }
}

impl From<Box<dyn Snapshot>> for GenericRawAccess<'_> {
    fn from(snapshot: Box<dyn Snapshot>) -> Self {
        GenericRawAccess::OwnedSnapshot(Rc::from(snapshot))
    }
}

impl<'a> From<&'a Fork> for GenericRawAccess<'a> {
    fn from(fork: &'a Fork) -> Self {
        GenericRawAccess::Fork(fork)
    }
}

impl From<Fork> for GenericRawAccess<'_> {
    fn from(fork: Fork) -> Self {
        GenericRawAccess::OwnedFork(Rc::new(fork))
    }
}

impl<'a> From<ReadonlyFork<'a>> for GenericRawAccess<'a> {
    fn from(ro_fork: ReadonlyFork<'a>) -> Self {
        GenericRawAccess::ReadonlyFork(ro_fork)
    }
}

impl From<OwnedReadonlyFork> for GenericRawAccess<'_> {
    fn from(ro_fork: OwnedReadonlyFork) -> Self {
        GenericRawAccess::OwnedReadonlyFork(ro_fork)
    }
}

impl AsReadonly for GenericRawAccess<'_> {
    type Readonly = Self;

    fn as_readonly(&self) -> Self::Readonly {
        use self::GenericRawAccess::*;

        match self {
            // Copy access for snapshots.
            Snapshot(snapshot) => Snapshot(*snapshot),
            OwnedSnapshot(snapshot) => OwnedSnapshot(Rc::clone(snapshot)),
            ReadonlyFork(ro_fork) => ReadonlyFork(*ro_fork),
            OwnedReadonlyFork(ro_fork) => OwnedReadonlyFork(ro_fork.clone()),

            // Translate access to readonly for forks.
            Fork(fork) => ReadonlyFork(fork.readonly()),
            OwnedFork(fork) => OwnedReadonlyFork(fork.as_readonly()),

            __NonExhaustive => unreachable!(),
        }
    }
}

/// Generic changes supported the database backend.
#[doc(hidden)] // should not be used directly by the client code
#[derive(Debug)]
pub enum GenericChanges<'a> {
    /// No changes.
    None,
    /// Immutable changes.
    Ref(ChangesRef<'a>),
    /// Mutable changes.
    Mut(ChangesMut<'a>),
}

impl ChangeSet for GenericChanges<'_> {
    fn as_ref(&self) -> Option<&ViewChanges> {
        match self {
            GenericChanges::None => None,
            GenericChanges::Ref(changes) => Some(&*changes),
            GenericChanges::Mut(changes) => Some(&*changes),
        }
    }

    fn as_mut(&mut self) -> Option<&mut ViewChanges> {
        match self {
            GenericChanges::None | GenericChanges::Ref(_) => None,
            GenericChanges::Mut(changes) => Some(&mut *changes),
        }
    }
}

impl<'a> RawAccess for GenericRawAccess<'a> {
    type Changes = GenericChanges<'a>;

    fn snapshot(&self) -> &dyn Snapshot {
        match self {
            GenericRawAccess::Snapshot(snapshot) => *snapshot,
            GenericRawAccess::OwnedSnapshot(snapshot) => snapshot.as_ref(),
            GenericRawAccess::Fork(fork) => fork.snapshot(),
            GenericRawAccess::OwnedFork(fork) => fork.snapshot(),
            GenericRawAccess::ReadonlyFork(ro_fork) => ro_fork.snapshot(),
            GenericRawAccess::OwnedReadonlyFork(ro_fork) => ro_fork.snapshot(),
            GenericRawAccess::__NonExhaustive => unreachable!(),
        }
    }

    fn changes(&self, address: &ResolvedAddress) -> Self::Changes {
        match self {
            GenericRawAccess::Snapshot(_) | GenericRawAccess::OwnedSnapshot(_) => {
                GenericChanges::None
            }
            GenericRawAccess::Fork(fork) => GenericChanges::Mut(fork.changes(address)),
            GenericRawAccess::OwnedFork(fork) => GenericChanges::Mut(fork.changes(address)),
            GenericRawAccess::ReadonlyFork(ro_fork) => {
                GenericChanges::Ref(ro_fork.changes(address))
            }
            GenericRawAccess::OwnedReadonlyFork(ro_fork) => {
                GenericChanges::Ref(ro_fork.changes(address))
            }
            GenericRawAccess::__NonExhaustive => unreachable!(),
        }
    }
}

/// Will panic in runtime if mutable methods are called on an inappropriate underlying access.
impl RawAccessMut for GenericRawAccess<'_> {}

/// Generic access containing any kind of accesses supported by the database.
///
/// This type is not intended to be exhaustively matched. It can be extended in the future
/// without breaking the semver compatibility.
#[derive(Debug, Clone)]
pub enum GenericAccess<T> {
    /// Access to the entire database.
    Raw(T),
    /// Prefixed access to the database.
    Prefixed(Prefixed<T>),
    /// Migration within a certain namespace.
    Migration(Migration<T>),
    /// Scratchpad for a migration.
    Scratchpad(Scratchpad<T>),

    /// Never actually generated.
    #[doc(hidden)]
    __NonExhaustive,
}

impl<T: RawAccess> From<T> for GenericAccess<T> {
    fn from(access: T) -> Self {
        GenericAccess::Raw(access)
    }
}

impl<T: RawAccess> From<Prefixed<T>> for GenericAccess<T> {
    fn from(access: Prefixed<T>) -> Self {
        GenericAccess::Prefixed(access)
    }
}

impl<T: RawAccess> From<Migration<T>> for GenericAccess<T> {
    fn from(access: Migration<T>) -> Self {
        GenericAccess::Migration(access)
    }
}

impl<T: RawAccess> From<Scratchpad<T>> for GenericAccess<T> {
    fn from(access: Scratchpad<T>) -> Self {
        GenericAccess::Scratchpad(access)
    }
}

impl<T: RawAccess> Access for GenericAccess<T> {
    type Base = T;

    fn get_index_metadata(
        self,
        addr: IndexAddress,
    ) -> Result<Option<IndexMetadata<Vec<u8>>>, AccessError> {
        match self {
            GenericAccess::Raw(access) => access.get_index_metadata(addr),
            GenericAccess::Prefixed(access) => access.get_index_metadata(addr),
            GenericAccess::Migration(access) => access.get_index_metadata(addr),
            GenericAccess::Scratchpad(access) => access.get_index_metadata(addr),
            GenericAccess::__NonExhaustive => unreachable!(),
        }
    }

    fn get_or_create_view(
        self,
        addr: IndexAddress,
        index_type: IndexType,
    ) -> Result<ViewWithMetadata<Self::Base>, AccessError> {
        match self {
            GenericAccess::Raw(access) => access.get_or_create_view(addr, index_type),
            GenericAccess::Prefixed(access) => access.get_or_create_view(addr, index_type),
            GenericAccess::Migration(access) => access.get_or_create_view(addr, index_type),
            GenericAccess::Scratchpad(access) => access.get_or_create_view(addr, index_type),
            GenericAccess::__NonExhaustive => unreachable!(),
        }
    }

    fn group_keys<K>(self, base_addr: IndexAddress) -> GroupKeys<Self::Base, K>
    where
        K: BinaryKey + ?Sized,
        Self::Base: AsReadonly<Readonly = Self::Base>,
    {
        match self {
            GenericAccess::Raw(access) => access.group_keys(base_addr),
            GenericAccess::Prefixed(access) => access.group_keys(base_addr),
            GenericAccess::Migration(access) => access.group_keys(base_addr),
            GenericAccess::Scratchpad(access) => access.group_keys(base_addr),
            GenericAccess::__NonExhaustive => unreachable!(),
        }
    }
}

/// Most generic access to the database, encapsulating any of base accesses and any of
/// possible access restrictions.
pub type ErasedAccess<'a> = GenericAccess<GenericRawAccess<'a>>;

impl ErasedAccess<'_> {
    /// Checks if the underlying access is mutable.
    pub fn is_mutable(&self) -> bool {
        match self {
            GenericAccess::Raw(access) => access.is_mutable(),
            GenericAccess::Prefixed(prefixed) => prefixed.access().is_mutable(),
            GenericAccess::Migration(migration) => migration.access().is_mutable(),
            GenericAccess::Scratchpad(scratchpad) => scratchpad.access().is_mutable(),
            GenericAccess::__NonExhaustive => unreachable!(),
        }
    }
}

/// Conversion to a most generic access to the database.
pub trait IntoErased<'a> {
    /// Performs the conversion.
    fn into_erased(self) -> ErasedAccess<'a>;
}

impl<'a> IntoErased<'a> for &'a dyn Snapshot {
    fn into_erased(self) -> ErasedAccess<'a> {
        GenericAccess::Raw(GenericRawAccess::from(self))
    }
}

impl<'a> IntoErased<'a> for &'a Fork {
    fn into_erased(self) -> ErasedAccess<'a> {
        GenericAccess::Raw(GenericRawAccess::from(self))
    }
}

impl<'a> IntoErased<'a> for ReadonlyFork<'a> {
    fn into_erased(self) -> ErasedAccess<'a> {
        GenericAccess::Raw(GenericRawAccess::from(self))
    }
}

impl<'a, T> IntoErased<'a> for Prefixed<T>
where
    T: Into<GenericRawAccess<'a>>,
{
    fn into_erased(self) -> ErasedAccess<'a> {
        let (prefix, access) = self.into_parts();
        let access: GenericRawAccess = access.into();
        GenericAccess::Prefixed(Prefixed::new(prefix, access))
    }
}

impl<'a, T> IntoErased<'a> for Migration<T>
where
    T: Into<GenericRawAccess<'a>>,
{
    fn into_erased(self) -> ErasedAccess<'a> {
        let (prefix, access) = self.into_parts();
        let access: GenericRawAccess = access.into();
        GenericAccess::Migration(Migration::new(prefix, access))
    }
}

impl<'a, T> IntoErased<'a> for Scratchpad<T>
where
    T: Into<GenericRawAccess<'a>>,
{
    fn into_erased(self) -> ErasedAccess<'a> {
        let (prefix, access) = self.into_parts();
        let access: GenericRawAccess = access.into();
        GenericAccess::Scratchpad(Scratchpad::new(prefix, access))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        access::{AccessExt, CopyAccessExt},
        Database, TemporaryDB,
    };

    #[test]
    fn generic_raw_access() {
        let db = TemporaryDB::new();
        let fork = db.fork();
        {
            let access = GenericRawAccess::from(&fork);
            assert!(access.is_mutable());
            let mut list = access.get_proof_list("list");
            list.extend(vec![1_u32, 2, 3]);
            access.get_entry("entry").set("!".to_owned());
        }
        {
            let access = GenericRawAccess::from(fork.readonly());
            assert!(!access.is_mutable());
            let list = access.get_proof_list::<_, u32>("list");
            assert_eq!(list.len(), 3);
            assert_eq!(list.iter().collect::<Vec<_>>(), vec![1, 2, 3]);

            let non_existent_map = access.get_map::<_, u32, u32>("map");
            assert_eq!(non_existent_map.get(&1), None);
            let non_existent_list = access.get_list::<_, u32>("other_list");
            assert_eq!(non_existent_list.len(), 0);
        }

        let patch = fork.into_patch();
        let access = GenericRawAccess::from(&patch as &dyn Snapshot);
        assert!(!access.is_mutable());
        assert_eq!(access.get_entry::<_, String>("entry").get().unwrap(), "!");

        db.merge(patch).unwrap();
        let snapshot = db.snapshot();
        let access = GenericRawAccess::from(snapshot.as_ref());
        assert!(!access.is_mutable());
        let list = access.get_proof_list::<_, u32>("list");
        assert_eq!(list.len(), 3);
        assert_eq!(list.iter().collect::<Vec<_>>(), vec![1, 2, 3]);

        // Accessing non-existent indexes should not result into a panic.
        let non_existent_map = access.get_map::<_, u32, u32>("map");
        assert_eq!(non_existent_map.get(&1), None);
        let non_existent_list = access.get_list::<_, u32>("other_list");
        assert_eq!(non_existent_list.len(), 0);
    }

    #[test]
    fn generic_raw_owned_access() {
        let db = TemporaryDB::new();
        let fork = db.fork();

        let access = GenericRawAccess::from(fork);
        assert!(access.is_mutable());
        {
            let mut list = access.get_proof_list("list");
            list.extend(vec![1_u32, 2, 3]);
            access.get_entry("entry").set("!".to_owned());
        }
        let fork = match access {
            GenericRawAccess::OwnedFork(fork) => Rc::try_unwrap(fork).unwrap(),
            _ => unreachable!(),
        };

        db.merge(fork.into_patch()).unwrap();
        let access = GenericRawAccess::from(db.snapshot());
        assert!(!access.is_mutable());
        let list = access.get_proof_list::<_, u32>("list");
        assert_eq!(list.len(), 3);
        assert_eq!(list.iter().collect::<Vec<_>>(), vec![1, 2, 3]);
        assert_eq!(access.get_entry::<_, String>("entry").get().unwrap(), "!");

        let non_existent_map = access.get_map::<_, u32, u32>("map");
        assert_eq!(non_existent_map.get(&1), None);
        let non_existent_list = access.get_list::<_, u32>("other_list");
        assert_eq!(non_existent_list.len(), 0);
    }

    #[test]
    #[should_panic(expected = "Attempt to modify a readonly view of the database")]
    fn generic_raw_access_panic_on_non_existing_index() {
        let db = TemporaryDB::new();
        let snapshot = db.snapshot();
        let access = GenericRawAccess::from(snapshot.as_ref());
        let mut list = access.get_list::<_, u32>("list");
        list.push(1); // should panic
    }

    #[test]
    #[should_panic(expected = "Attempt to modify a readonly view of the database")]
    fn generic_raw_access_panic_on_existing_index() {
        let db = TemporaryDB::new();
        let fork = db.fork();
        fork.get_entry("entry").set(1_u8);
        let access = GenericRawAccess::from(fork.readonly());
        access.get_entry("entry").set(2_u8); // should panic
    }

    #[test]
    #[should_panic(expected = "Attempt to modify a readonly view of the database")]
    fn generic_raw_access_as_readonly() {
        let db = TemporaryDB::new();
        let fork = db.fork();
        fork.get_proof_list("list").extend(vec![1_u32, 2, 3]);
        let access = GenericRawAccess::from(&fork);
        let readonly = access.as_readonly();
        assert!(!readonly.is_mutable());
        let mut list = readonly.get_proof_list::<_, u32>("list");
        assert_eq!(list.len(), 3);
        assert_eq!(list.get(1), Some(2));
        list.push(4); // should panic
    }

    #[test]
    #[should_panic(expected = "Attempt to modify a readonly view of the database")]
    fn generic_raw_access_as_static_readonly() {
        let db = TemporaryDB::new();
        let fork = db.fork();
        fork.get_proof_list("list").extend(vec![1_u32, 2, 3]);
        let access = GenericRawAccess::from(fork);
        let readonly = access.as_readonly();
        assert!(!readonly.is_mutable());
        let mut list = readonly.get_proof_list::<_, u32>("list");
        assert_eq!(list.len(), 3);
        assert_eq!(list.get(1), Some(2));
        list.push(4); // should panic
    }

    #[test]
    fn generic_access_workflow() {
        let db = TemporaryDB::new();
        let fork = db.fork();

        let access = Prefixed::new("foo", &fork).into_erased();
        assert!(access.is_mutable());
        access.get_list("list").extend(vec![2_u32, 3, 4]);
        access.get_proof_map("map").put("foo", 42_u64);
        access.get_value_set("set").insert(100_u8);

        // Check that elements are available from the underlying fork.
        let access = (&fork).into_erased();
        assert!(access.is_mutable());
        assert_eq!(access.get_list::<_, u32>("foo.list").len(), 3);
        assert_eq!(
            access.get_proof_map::<_, str, u64>("foo.map").get("foo"),
            Some(42)
        );
        assert!(access.get_value_set::<_, u8>("foo.set").contains(&100));

        // ...or from `Prefixed<ReadonlyFork>`.
        let access = Prefixed::new("foo", fork.readonly()).into_erased();
        assert!(!access.is_mutable());
        assert_eq!(access.get_list::<_, u32>("list").len(), 3);
        assert_eq!(
            access.get_proof_map::<_, str, u64>("map").get("foo"),
            Some(42)
        );
        assert!(access.get_value_set::<_, u8>("set").contains(&100));

        // Erased access can also be used to modify data.
        let access = Migration::new("foo", &fork).into_erased();
        assert!(access.is_mutable());
        access.get_proof_list("list").extend(vec![4_i32, 5, 6, 7]);
        access.get_key_set("set").insert(&99_u8);
        let access = Scratchpad::new("foo", &fork).into_erased();
        access.get_entry("iter_position").set(123_u32);
        drop(access);

        let patch = fork.into_patch();
        let patch_ref = &patch as &dyn Snapshot;
        let access = Migration::new("foo", patch_ref).into_erased();
        assert!(!access.is_mutable());
        let list = access.get_proof_list::<_, i32>("list");
        assert_eq!(list.len(), 4);
        assert_eq!(list.iter().collect::<Vec<_>>(), vec![4, 5, 6, 7]);
        let set = access.get_key_set::<_, u8>("set");
        assert_eq!(set.iter().collect::<Vec<_>>(), vec![99]);

        let erased = Scratchpad::new("foo", patch_ref).into_erased();
        assert!(!access.is_mutable());
        assert_eq!(erased.get_entry::<_, u32>("iter_position").get(), Some(123));
    }
}
