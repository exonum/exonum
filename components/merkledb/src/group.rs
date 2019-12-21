use std::marker::PhantomData;

pub use crate::views::GroupKeys;

use crate::{
    access::{Access, AccessError, FromAccess, RawAccess},
    BinaryKey, IndexAddress,
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
/// #     access::{Access, AccessExt, FromAccess},
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
/// # use exonum_merkledb::{access::{AccessExt, FromAccess}, Database, Group, ListIndex, TemporaryDB};
/// let db = TemporaryDB::new();
/// let fork = db.fork();
/// let group: Group<_, u64, ListIndex<_, u64>> =
///     FromAccess::from_access(&fork, "group".into()).unwrap();
/// group.get(&1).push(1);
/// group.get(&2).extend(vec![1, 2, 3]);
/// // Members of the group can be accessed independently.
/// assert_eq!(fork.get_list::<_, u64>(("group", &2_u64)).len(), 3);
/// ```
///
/// Group keys can be unsized:
///
/// ```
/// # use exonum_merkledb::{access::AccessExt, Database, Group, ListIndex, TemporaryDB};
/// # let db = TemporaryDB::new();
/// # let fork = db.fork();
/// let group: Group<_, str, ListIndex<_, u64>> = fork.get_group("unsized_group");
/// group.get("foo").push(1);
/// group.get("bar").push(42);
/// # assert_eq!(fork.readonly().get_list::<_, u64>(("unsized_group", "bar")).len(), 1);
/// ```
///
/// This example shows incorrect use of the [`keys`] iterator:
///
/// ```should_panic
/// # use exonum_merkledb::{access::AccessExt, Database, Group, ListIndex, TemporaryDB};
/// let db = TemporaryDB::new();
/// let fork = db.fork();
/// let group: Group<_, str, ListIndex<_, String>> = fork.get_group("group");
/// group.get("foo").push("foo".to_owned());
/// group.get("bar").push("bar".to_owned());
///
/// for key in group.keys() {
///     fork.get_list("list").push(key); // << will panic
/// }
/// ```
///
/// In this case, the fix is easy: just move the index creation outside the `for` cycle.
///
/// ```
/// # use exonum_merkledb::{access::AccessExt, Database, Group, ListIndex, TemporaryDB};
/// # let db = TemporaryDB::new();
/// # let fork = db.fork();
/// # let group: Group<_, str, ListIndex<_, String>> = fork.get_group("group");
/// # group.get("foo").push("foo".to_owned());
/// # group.get("bar").push("bar".to_owned());
/// let mut list = fork.get_list("list");
/// for key in group.keys() {
///     list.push(key);
/// }
/// // ...or, more idiomatically:
/// //list.extend(group.keys());
/// ```
///
/// [`keys`]: #method.keys
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

    /// Iterates over keys of indexes in this group and collects them to a vector.
    /// Compared to `keys`, this operation is safer, but involves memory overhead.
    pub fn buffered_keys(&self) -> Vec<K::Owned> {
        self.keys().collect()
    }

    /// Iterates over keys of indexes in this group.
    ///
    /// # Panics
    ///
    /// If the group is built on top a [`Fork`], an attempt to create an index from the fork
    /// while iterating over keys will result in a panic. This is because such an operation
    /// may invalidate the iterator. As a workaround, consider using the [`buffered_keys`]
    /// method or otherwise delay the DB modification after the iterator is dropped.
    ///
    /// [`Fork`]: struct.Fork.html
    /// [`buffered_keys`]: #method.buffered_keys
    pub fn keys(&self) -> Keys<T::Base, K> {
        let inner = self.access.clone().group_keys(self.prefix.clone());
        Keys {
            inner,
            _key: PhantomData,
        }
    }
}

/// Iterator over keys in a group.
#[derive(Debug)]
pub struct Keys<T: RawAccess, K: ?Sized> {
    inner: GroupKeys<T>,
    _key: PhantomData<K>,
}

impl<T, K> Iterator for Keys<T, K>
where
    T: RawAccess,
    K: BinaryKey + ?Sized,
{
    type Item = K::Owned;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(K::read)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        access::{AccessExt, Prefixed, RawAccessMut},
        migration::Migration,
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

    fn test_key_iter<A>(fork: A)
    where
        A: Access,
        A::Base: RawAccessMut,
    {
        {
            let group: Group<_, str, ProofListIndex<_, String>> = fork.clone().get_group("group");
            group.get("foo").push("foo".to_owned());
            group.get("bar").push("bar".to_owned());
            group.get("baz").push("baz".to_owned());
        }
        {
            let group: Group<_, u32, ProofListIndex<_, String>> =
                Group::from_access(fork.clone(), ("prefixed", &0_u8).into()).unwrap();
            group.get(&1).push("foo".to_owned());
            group.get(&2).push("bar".to_owned());
            group.get(&5).push("baz".to_owned());
            group.get(&100_000).push("?".to_owned());
        }

        // Add some unrelated stuff to the DB.
        fork.clone().get_entry("gr").set(42);
        fork.clone().get_entry("group_").set("!".to_owned());
        fork.clone()
            .get_list(("group_", &1_u8))
            .extend(vec![1, 2, 3]);
        fork.clone().get_entry("prefix").set(".".to_owned());
        fork.clone().get_entry("prefixed").set("??".to_owned());
        fork.clone().get_list(("prefixed", &1_u8)).push(42);
        fork.clone()
            .get_entry(("prefixed", &concat_keys!(&1_u8, &42_u32)))
            .set(42);
        fork.clone().get_entry("t").set(21);
        fork.clone().get_entry("unrelated").set(23);

        let group: Group<_, str, ProofListIndex<_, String>> = fork.clone().get_group("group");
        assert_eq!(
            group.keys().collect::<Vec<_>>(),
            vec!["bar".to_owned(), "baz".to_owned(), "foo".to_owned()]
        );
        assert_eq!(
            group.buffered_keys(),
            vec!["bar".to_owned(), "baz".to_owned(), "foo".to_owned()]
        );

        let group: Group<_, u32, ProofListIndex<_, String>> =
            Group::from_access(fork, ("prefixed", &0_u8).into()).unwrap();;
        assert_eq!(group.keys().collect::<Vec<_>>(), vec![1, 2, 5, 100_000]);
        assert_eq!(group.buffered_keys(), vec![1, 2, 5, 100_000]);
    }

    #[test]
    fn iterating_over_keys() {
        let db = TemporaryDB::new();
        let fork = db.fork();
        test_key_iter(&fork);

        {
            let group: Group<_, str, ProofListIndex<_, String>> =
                fork.readonly().get_group("group");
            assert_eq!(
                group.keys().collect::<Vec<_>>(),
                vec!["bar".to_owned(), "baz".to_owned(), "foo".to_owned()]
            );

            let group: Group<_, u32, ProofListIndex<_, String>> =
                Group::from_access(fork.readonly(), ("prefixed", &0_u8).into()).unwrap();
            assert_eq!(group.keys().collect::<Vec<_>>(), vec![1, 2, 5, 100_000]);
        }

        let patch = fork.into_patch();
        let group: Group<_, str, ProofListIndex<_, String>> = patch.get_group("group");
        assert_eq!(
            group.keys().collect::<Vec<_>>(),
            vec!["bar".to_owned(), "baz".to_owned(), "foo".to_owned()]
        );

        let group: Group<_, u32, ProofListIndex<_, String>> =
            Group::from_access(&patch, ("prefixed", &0_u8).into()).unwrap();
        assert_eq!(group.keys().collect::<Vec<_>>(), vec![1, 2, 5, 100_000]);
    }

    #[test]
    fn iterating_over_keys_in_prefixed_access() {
        let db = TemporaryDB::new();
        let fork = db.fork();
        test_key_iter(Prefixed::new("namespace", &fork));
    }

    #[test]
    fn iterating_over_keys_in_migration() {
        let db = TemporaryDB::new();
        let fork = db.fork();
        test_key_iter(Migration::new("namespace", &fork));
    }
}
