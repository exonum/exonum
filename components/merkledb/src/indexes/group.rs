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

use std::marker::PhantomData;

use crate::{
    access::{Access, AccessError, FromAccess},
    views::{AsReadonly, GroupKeys, IndexAddress},
    BinaryKey,
};

// cspell:ignore foob

/// Group of indexes distinguished by a prefix.
///
/// All indexes in the group have the same type. Indexes are initialized lazily;
/// i.e., no initialization is performed when the group is created.
///
/// # Safety
///
/// Using a group within a group (including indirectly via components) can lead to index collision
/// if the keys in both groups have variable length (e.g., keys are strings). A collision in turn
/// may result in logical errors and data corruption. For example:
///
/// ```
/// # use exonum_merkledb::{
/// #     access::{Access, CopyAccessExt, FromAccess},
/// #     Database, Group, ListIndex, TemporaryDB,
/// # };
/// type StrGroup<T> = Group<T, str, ListIndex<<T as Access>::Base, u64>>;
///
/// let db = TemporaryDB::new();
/// let fork = db.fork();
/// let outer_group: Group<_, str, StrGroup<_>> =
///     FromAccess::from_access(&fork, "group".into()).unwrap();
/// outer_group.get("foo").get("bar").extend(vec![1, 2]);
/// outer_group.get("foob").get("ar").push(3);
/// // Both accessed lists have the same address and thus share the same data:
/// assert_eq!(
///     fork.get_list(("group", "foobar")).iter().collect::<Vec<u64>>(),
///     vec![1, 2, 3]
/// );
/// ```
///
/// # Examples
///
/// ```
/// # use exonum_merkledb::{access::{CopyAccessExt, FromAccess}, Database, Group, ListIndex, TemporaryDB};
/// let db = TemporaryDB::new();
/// let fork = db.fork();
/// let group: Group<_, u64, ListIndex<_, u64>> = fork.get_group("group");
/// group.get(&1).push(1);
/// group.get(&2).extend(vec![1, 2, 3]);
/// // Members of the group can be accessed independently.
/// assert_eq!(fork.get_list::<_, u64>(("group", &2_u64)).len(), 3);
///
/// // It is possible to enumerate keys in the group, but only if the underlying access
/// // is readonly.
/// let group: Group<_, u64, ListIndex<_, u64>> = fork.readonly().get_group("group");
/// assert_eq!(group.keys().collect::<Vec<_>>(), vec![1, 2]);
/// ```
///
/// Group keys can be unsized:
///
/// ```
/// # use exonum_merkledb::{access::CopyAccessExt, Database, Group, ListIndex, TemporaryDB};
/// # let db = TemporaryDB::new();
/// # let fork = db.fork();
/// let group: Group<_, str, ListIndex<_, u64>> = fork.get_group("unsized_group");
/// group.get("foo").push(1);
/// group.get("bar").push(42);
/// # assert_eq!(fork.readonly().get_list::<_, u64>(("unsized_group", "bar")).len(), 1);
/// ```
#[derive(Debug)]
pub struct Group<T, K: ?Sized, I> {
    access: T,
    prefix: IndexAddress,
    _key: PhantomData<K>,
    _index: PhantomData<I>,
}

impl<T, K, I> FromAccess<T> for Group<T, K, I>
where
    T: Access,
    K: BinaryKey + ?Sized,
    I: FromAccess<T>,
{
    fn from_access(access: T, addr: IndexAddress) -> Result<Self, AccessError> {
        Ok(Self {
            access,
            prefix: addr,
            _key: PhantomData,
            _index: PhantomData,
        })
    }
}

impl<T, K, I> Group<T, K, I>
where
    T: Access,
    K: BinaryKey + ?Sized,
    I: FromAccess<T>,
{
    /// Gets an index corresponding to the specified key.
    ///
    /// # Panics
    ///
    /// If the index is present and has a wrong type.
    pub fn get(&self, key: &K) -> I {
        let addr = self.prefix.clone().append_key(key);
        I::from_access(self.access.clone(), addr)
            .unwrap_or_else(|e| panic!("MerkleDB error: {}", e))
    }
}

impl<T, K, I> Group<T, K, I>
where
    T: Access,
    T::Base: AsReadonly<Readonly = T::Base>,
    K: BinaryKey + ?Sized,
{
    /// Iterator over keys in this group.
    ///
    /// The iterator buffers keys in memory and may become inconsistent. Although
    /// the Rust type system prevents iterating over keys in a group based on [`Fork`],
    /// it it still possible to make the iterator return inconsistent results. Indeed,
    /// for a group is based on [`ReadonlyFork`], it is possible to add new indexes via `Fork`
    /// while the iteration is in progress.
    ///
    /// For this reason, it is advised to use this method for groups based on `ReadonlyFork`
    /// only in the case where stale reads are tolerated or are prevented on the application level.
    /// Groups based on [`Snapshot`] implementations (including [`Patch`]es) are not affected
    /// by this issue.
    ///
    /// [`Fork`]: ../struct.Fork.html
    /// [`ReadonlyFork`]: ../struct.ReadonlyFork.html
    /// [`Snapshot`]: ../trait.Snapshot.html
    /// [`Patch`]: ../struct.Patch.html
    pub fn keys(&self) -> GroupKeys<T::Base, K> {
        self.access.clone().group_keys(self.prefix.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        access::{AccessExt, CopyAccessExt, Prefixed, RawAccessMut},
        migration::{Migration, Scratchpad},
        Database, ProofListIndex, TemporaryDB,
    };

    #[test]
    fn group() {
        let db = TemporaryDB::new();
        let fork = db.fork();

        {
            let group: Group<_, u32, ProofListIndex<_, String>> = fork.get_group("group");
            let mut list = group.get(&1);
            list.push("foo".to_owned());
            list.push("bar".to_owned());
            group.get(&2).push("baz".to_owned());
        }

        {
            let list = fork.get_proof_list::<_, String>(("group", &1_u32));
            assert_eq!(list.len(), 2);
            assert_eq!(list.get(1), Some("bar".to_owned()));
            let other_list = fork.get_proof_list::<_, String>(("group", &2_u32));
            assert_eq!(other_list.len(), 1);
        }

        db.merge_sync(fork.into_patch()).unwrap();
        let snapshot = db.snapshot();
        let group: Group<_, u32, ProofListIndex<_, String>> = snapshot.get_group("group");
        assert_eq!(group.get(&1).len(), 2);
        assert_eq!(group.get(&2).len(), 1);
        assert!(group.get(&0).is_empty());

        // The next line fails to compile because `Snapshot` cannot be written to:
        // group.get(&3).push("quux".to_owned());
    }

    fn prepare_key_iter<A>(fork: &A)
    where
        A: Access,
        A::Base: RawAccessMut,
    {
        let group: Group<_, str, ProofListIndex<_, String>> = fork.get_group("group");
        group.get("foo").push("foo".to_owned());
        group.get("bar").push("bar".to_owned());
        group.get("baz").push("baz".to_owned());

        let group: Group<_, u32, ProofListIndex<_, String>> =
            Group::from_access(fork.clone(), ("prefixed", &0_u8).into()).unwrap();
        group.get(&1).push("foo".to_owned());
        group.get(&2).push("bar".to_owned());
        group.get(&5).push("baz".to_owned());
        group.get(&100_000).push("?".to_owned());

        // Add some unrelated stuff to the DB.
        fork.get_entry("gr").set(42);
        fork.get_entry("group_").set("!".to_owned());
        fork.get_list(("group_", &1_u8)).extend(vec![1, 2, 3]);
        fork.get_entry("prefix").set(".".to_owned());
        fork.get_entry("prefixed").set("??".to_owned());
        fork.get_list(("prefixed", &1_u8)).push(42);
        fork.get_entry(("prefixed", &concat_keys!(&1_u8, &42_u32)))
            .set(42);
        fork.get_entry("t").set(21);
        fork.get_entry("unrelated").set(23);
    }

    fn test_key_iter<A>(snapshot: A)
    where
        A: Access,
        A::Base: AsReadonly<Readonly = A::Base>,
    {
        let group: Group<_, str, ProofListIndex<_, String>> = snapshot.get_group("group");
        assert_eq!(
            group.keys().collect::<Vec<_>>(),
            vec!["bar".to_owned(), "baz".to_owned(), "foo".to_owned()]
        );

        let group: Group<_, u32, ProofListIndex<_, String>> =
            Group::from_access(snapshot, ("prefixed", &0_u8).into()).unwrap();
        assert_eq!(group.keys().collect::<Vec<_>>(), vec![1, 2, 5, 100_000]);
    }

    #[test]
    fn iterating_over_keys() {
        let db = TemporaryDB::new();
        let fork = db.fork();
        prepare_key_iter(&&fork);
        test_key_iter(fork.readonly());
        let patch = fork.into_patch();
        test_key_iter(&patch);
    }

    #[test]
    fn iterating_over_keys_in_prefixed_access() {
        let db = TemporaryDB::new();
        let fork = db.fork();
        prepare_key_iter(&Prefixed::new("namespace", &fork));
        test_key_iter(Prefixed::new("namespace", fork.readonly()));
        let patch = fork.into_patch();
        test_key_iter(Prefixed::new("namespace", &patch));
        db.merge(patch).unwrap();
        test_key_iter(Prefixed::new("namespace", &db.snapshot()));
    }

    #[test]
    fn iterating_over_keys_in_migration() {
        let db = TemporaryDB::new();
        let fork = db.fork();
        prepare_key_iter(&Migration::new("namespace", &fork));
        test_key_iter(Migration::new("namespace", fork.readonly()));
        let patch = fork.into_patch();
        test_key_iter(Migration::new("namespace", &patch));
        db.merge(patch).unwrap();
        test_key_iter(Migration::new("namespace", &db.snapshot()));
    }

    #[test]
    fn iterating_over_keys_in_scratchpad() {
        let db = TemporaryDB::new();
        let fork = db.fork();
        prepare_key_iter(&Scratchpad::new("namespace", &fork));
        test_key_iter(Scratchpad::new("namespace", fork.readonly()));
        let patch = fork.into_patch();
        test_key_iter(Scratchpad::new("namespace", &patch));
        db.merge(patch).unwrap();
        test_key_iter(Scratchpad::new("namespace", &db.snapshot()));
    }
}
