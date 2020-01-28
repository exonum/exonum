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

use crate::{
    access::{Access, AccessError, AsReadonly, Prefixed},
    db::{ChangesMut, ChangesRef, ViewChanges},
    migration::{Migration, Scratchpad},
    views::{ChangeSet, GroupKeys, IndexMetadata, RawAccess, RawAccessMut, ViewWithMetadata},
    BinaryKey, Fork, IndexAddress, IndexType, ReadonlyFork, ResolvedAddress, Snapshot,
};

#[derive(Debug, Clone, Copy)]
pub enum GenericRawAccess<'a> {
    Snapshot(&'a dyn Snapshot),
    Fork(&'a Fork),
    ReadonlyFork(ReadonlyFork<'a>),
}

impl GenericRawAccess<'_> {
    pub fn is_mutable(self) -> bool {
        match self {
            GenericRawAccess::Fork(_) => true,
            _ => false,
        }
    }
}

impl<'a> From<&'a dyn Snapshot> for GenericRawAccess<'a> {
    fn from(snapshot: &'a dyn Snapshot) -> Self {
        GenericRawAccess::Snapshot(snapshot)
    }
}

impl<'a> From<&'a Fork> for GenericRawAccess<'a> {
    fn from(fork: &'a Fork) -> Self {
        GenericRawAccess::Fork(fork)
    }
}

impl<'a> From<ReadonlyFork<'a>> for GenericRawAccess<'a> {
    fn from(ro_fork: ReadonlyFork<'a>) -> Self {
        GenericRawAccess::ReadonlyFork(ro_fork)
    }
}

#[derive(Debug)]
pub enum GenericChanges<'a> {
    None,
    Ref(ChangesRef<'a>),
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
            GenericRawAccess::Fork(fork) => fork.snapshot(),
            GenericRawAccess::ReadonlyFork(ro_fork) => ro_fork.snapshot(),
        }
    }

    fn changes(&self, address: &ResolvedAddress) -> Self::Changes {
        match self {
            GenericRawAccess::Snapshot(_) => GenericChanges::None,
            GenericRawAccess::Fork(fork) => GenericChanges::Mut(fork.changes(address)),
            GenericRawAccess::ReadonlyFork(ro_fork) => {
                GenericChanges::Ref(ro_fork.changes(address))
            }
        }
    }
}

/// Will panic in runtime if mutable methods are called on an inappropriate underlying access.
impl RawAccessMut for GenericRawAccess<'_> {}

#[derive(Debug, Clone)]
pub enum GenericAccess<T> {
    Raw(T),
    Prefixed(Prefixed<T>),
    Migration(Migration<T>),
    Scratchpad(Scratchpad<T>),
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
        }
    }
}

pub trait IntoGeneric<'a> {
    fn into_generic(self) -> GenericAccess<GenericRawAccess<'a>>;
}

impl<'a> IntoGeneric<'a> for &'a dyn Snapshot {
    fn into_generic(self) -> GenericAccess<GenericRawAccess<'a>> {
        GenericAccess::Raw(GenericRawAccess::from(self))
    }
}

impl<'a> IntoGeneric<'a> for &'a Fork {
    fn into_generic(self) -> GenericAccess<GenericRawAccess<'a>> {
        GenericAccess::Raw(GenericRawAccess::from(self))
    }
}

impl<'a> IntoGeneric<'a> for ReadonlyFork<'a> {
    fn into_generic(self) -> GenericAccess<GenericRawAccess<'a>> {
        GenericAccess::Raw(GenericRawAccess::from(self))
    }
}

impl<'a, T> IntoGeneric<'a> for Prefixed<T>
where
    T: Into<GenericRawAccess<'a>>,
{
    fn into_generic(self) -> GenericAccess<GenericRawAccess<'a>> {
        let (prefix, access) = self.into_parts();
        let access: GenericRawAccess = access.into();
        GenericAccess::Prefixed(Prefixed::new(prefix, access))
    }
}

impl<'a, T> IntoGeneric<'a> for Migration<T>
where
    T: Into<GenericRawAccess<'a>>,
{
    fn into_generic(self) -> GenericAccess<GenericRawAccess<'a>> {
        let (prefix, access) = self.into_parts();
        let access: GenericRawAccess = access.into();
        GenericAccess::Migration(Migration::new(prefix, access))
    }
}

impl<'a, T> IntoGeneric<'a> for Scratchpad<T>
where
    T: Into<GenericRawAccess<'a>>,
{
    fn into_generic(self) -> GenericAccess<GenericRawAccess<'a>> {
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
    fn generic_access_workflow() {
        let db = TemporaryDB::new();
        let fork = db.fork();

        let access = Prefixed::new("foo", &fork).into_generic();
        access.get_list("list").extend(vec![2_u32, 3, 4]);
        access.get_proof_map("map").put("foo", 42_u64);
        access.get_value_set("set").insert(100_u8);

        // Check that elements are available from the underlying fork.
        let access = (&fork).into_generic();
        assert_eq!(access.get_list::<_, u32>("foo.list").len(), 3);
        assert_eq!(
            access.get_proof_map::<_, str, u64>("foo.map").get("foo"),
            Some(42)
        );
        assert!(access.get_value_set::<_, u8>("foo.set").contains(&100));

        // ...or from `Prefixed<ReadonlyFork>`.
        let access = Prefixed::new("foo", fork.readonly()).into_generic();
        assert_eq!(access.get_list::<_, u32>("list").len(), 3);
        assert_eq!(
            access.get_proof_map::<_, str, u64>("map").get("foo"),
            Some(42)
        );
        assert!(access.get_value_set::<_, u8>("set").contains(&100));

        // Erased access can also be used to modify data.
        let access = Migration::new("foo", &fork).into_generic();
        access.get_proof_list("list").extend(vec![4_i32, 5, 6, 7]);
        access.get_key_set("set").insert(99_u8);
        let access = Scratchpad::new("foo", &fork).into_generic();
        access.get_entry("iter_position").set(123_u32);
        drop(access);

        let patch = fork.into_patch();
        let patch_ref = &patch as &dyn Snapshot;
        let access = Migration::new("foo", patch_ref).into_generic();
        let list = access.get_proof_list::<_, i32>("list");
        assert_eq!(list.len(), 4);
        assert_eq!(list.iter().collect::<Vec<_>>(), vec![4, 5, 6, 7]);
        let set = access.get_key_set::<_, u8>("set");
        assert_eq!(set.iter().collect::<Vec<_>>(), vec![99]);

        let erased = Scratchpad::new("foo", patch_ref).into_generic();
        assert_eq!(erased.get_entry::<_, u32>("iter_position").get(), Some(123));
    }
}
