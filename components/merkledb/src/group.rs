use std::marker::PhantomData;

use crate::views::{IndexMetadata, IndexesPoolWrapper, RawAccess, View, ViewWithMetadata};
use crate::{
    access::{Access, AccessError, FromAccess},
    views::IndexAddress,
    BinaryKey, BinaryValue, IndexType, ResolvedAddress,
};
use byteorder::{LittleEndian, WriteBytesExt};
use failure::Fail;
use std::borrow::Borrow;
use std::{borrow::Cow, fmt, num::NonZeroU64};

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
    K: BinaryKey + ?Sized + fmt::Debug,
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
    T: RawAccess,
    K: BinaryKey + Clone, //TODO: This clone definitely should be removed.
    I: FromAccess<T>,
{
    pub fn iter<V: BinaryValue>(&self) -> GroupIter<K, T, I> {
        let prefix = IndexAddress::from(self.prefix.clone());
        let indexes_pool = IndexesPoolWrapper::new(self.access.clone());
        let keys = indexes_pool.suffixes(&prefix);

        GroupIter::new(self.prefix.clone(), keys, self.access.clone())
    }
}

pub struct GroupIter<K, T, I> {
    base: IndexAddress,
    keys: Vec<K>,
    access: T,
    _index: PhantomData<I>,
}

impl<K, T, I> GroupIter<K, T, I>
where
    T: Access,
    K: BinaryKey,
    I: FromAccess<T>,
{
    fn new(base: IndexAddress, keys: Vec<K>, access: T) -> Self {
        Self {
            base,
            keys,
            access,
            _index: PhantomData,
        }
    }
}

impl<K, T, I> Iterator for GroupIter<K, T, I>
where
    T: Access,
    K: BinaryKey,
    I: FromAccess<T>,
{
    type Item = I;

    fn next(&mut self) -> Option<Self::Item> {
        if self.keys.is_empty() {
            return None;
        }

        let key = self.keys.remove(0);
        let addr = self.base.clone().append_key(&key);
        let index = I::from_access(self.access.clone(), addr)
            .unwrap_or_else(|e| panic!("MerkleDB error: {}", e));

        Some(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{access::AccessExt, Database, ProofListIndex, TemporaryDB};

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

    #[test]
    fn group_iter() {
        const GROUP_NAME: &'static str = "group_group";

        let db = TemporaryDB::new();
        let fork = db.fork();

        {
            let group: Group<_, String, ProofListIndex<_, String>> = fork.get_group(GROUP_NAME);
            let mut list = group.get(&"g1".to_owned());
            list.push("foo".to_owned());
            group.get(&"g2".to_owned()).push("bar".to_owned());
            group.get(&"g3".to_owned()).push("baz".to_owned());
        }

        db.merge(fork.into_patch());

        let snapshot = db.snapshot();
        let group: Group<_, String, ProofListIndex<_, String>> = snapshot.get_group(GROUP_NAME);
        let indexes = group.iter::<String>();

        let mut results = indexes
            .map(|index| index.get(0).unwrap())
            .collect::<Vec<_>>();

        assert_eq!(
            results,
            vec!["foo".to_owned(), "bar".to_owned(), "baz".to_owned()]
        )
    }
}
