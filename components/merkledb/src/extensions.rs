//! Extension traits to simplify index instantiation.

use std::borrow::Cow;

use crate::{
    validation::assert_valid_name,
    views::{IndexType, ViewWithMetadata},
    BinaryKey, BinaryValue, Entry, IndexAddress, KeySetIndex, ListIndex, MapIndex, ObjectHash,
    ProofListIndex, ProofMapIndex, RawAccess, RawAccessMut, SparseListIndex, ValueSetIndex,
};

/// Extension trait allowing for easy access to indices from any type implementing
/// `IndexAccess`.
pub trait Access {
    /// Index access serving as the basis for created indices.
    type Base: RawAccess;

    /// Gets a generic `View` with the specified address.
    fn get_view(&self, addr: IndexAddress) -> Option<ViewWithMetadata<Self::Base>>;

    /// Gets or creates a generic `View` with the specified address.
    fn get_or_create_view(
        &self,
        addr: IndexAddress,
        index_type: IndexType,
    ) -> ViewWithMetadata<Self::Base>
    where
        Self::Base: RawAccessMut;

    /// Gets an entry index with the specified address.
    ///
    /// # Panics
    ///
    /// If the index exists, but is not an entry.
    fn entry<I, V>(&self, addr: I) -> Option<Entry<Self::Base, V>>
    where
        I: Into<IndexAddress>,
        V: BinaryValue,
    {
        self.get_view(addr.into()).map(Entry::new)
    }

    /// Gets a list index with the specified address.
    ///
    /// # Panics
    ///
    /// If the index exists, but is not a list.
    fn list<I, V>(&self, addr: I) -> Option<ListIndex<Self::Base, V>>
    where
        I: Into<IndexAddress>,
        V: BinaryValue,
    {
        self.get_view(addr.into()).map(ListIndex::new)
    }

    /// Gets a map index with the specified address.
    ///
    /// # Panics
    ///
    /// If the index exists, but is not a map.
    fn map<I, K, V>(&self, addr: I) -> Option<MapIndex<Self::Base, K, V>>
    where
        I: Into<IndexAddress>,
        K: BinaryKey,
        V: BinaryValue,
    {
        self.get_view(addr.into()).map(MapIndex::new)
    }

    /// Gets a Merkelized list index with the specified address.
    ///
    /// # Panics
    ///
    /// If the index exists, but is not a Merkelized list.
    fn proof_list<I, V>(&self, addr: I) -> Option<ProofListIndex<Self::Base, V>>
    where
        I: Into<IndexAddress>,
        V: BinaryValue,
    {
        self.get_view(addr.into()).map(ProofListIndex::new)
    }

    /// Gets a Merkelized list index with the specified address.
    ///
    /// # Panics
    ///
    /// If the index exists, but is not a Merkelized list.
    fn proof_map<I, K, V>(&self, addr: I) -> Option<ProofMapIndex<Self::Base, K, V>>
    where
        I: Into<IndexAddress>,
        K: BinaryKey + ObjectHash,
        V: BinaryValue,
    {
        self.get_view(addr.into()).map(ProofMapIndex::new)
    }

    /// Gets a sparse list index with the specified address.
    ///
    /// # Panics
    ///
    /// If the index exists, but is not a sparse list.
    fn sparse_list<I, V>(&self, addr: I) -> Option<SparseListIndex<Self::Base, V>>
    where
        I: Into<IndexAddress>,
        V: BinaryValue,
    {
        self.get_view(addr.into()).map(SparseListIndex::new)
    }

    /// Gets a key set index with the specified address.
    ///
    /// # Panics
    ///
    /// If the index exists, but is not a key set.
    fn key_set<I, V>(&self, addr: I) -> Option<KeySetIndex<Self::Base, V>>
    where
        I: Into<IndexAddress>,
        V: BinaryKey,
    {
        self.get_view(addr.into()).map(KeySetIndex::new)
    }

    /// Gets a value set index with the specified address.
    ///
    /// # Panics
    ///
    /// If the index exists, but is not a value set.
    fn value_set<I, V>(&self, addr: I) -> Option<ValueSetIndex<Self::Base, V>>
    where
        I: Into<IndexAddress>,
        V: BinaryValue + ObjectHash,
    {
        self.get_view(addr.into()).map(ValueSetIndex::new)
    }

    /// Gets or creates an entry index with the specified address.
    ///
    /// # Panics
    ///
    /// If the index exists, but is not an entry.
    fn ensure_entry<I, V>(&self, addr: I) -> Entry<Self::Base, V>
    where
        I: Into<IndexAddress>,
        V: BinaryValue,
        Self::Base: RawAccessMut,
    {
        let view = self.get_or_create_view(addr.into(), IndexType::Entry);
        Entry::new(view)
    }

    /// Gets or creates a list index with the specified address.
    ///
    /// # Panics
    ///
    /// If the index already exists and is not a list.
    fn ensure_list<I, V>(&self, addr: I) -> ListIndex<Self::Base, V>
    where
        I: Into<IndexAddress>,
        V: BinaryValue,
        Self::Base: RawAccessMut,
    {
        let view = self.get_or_create_view(addr.into(), IndexType::List);
        ListIndex::new(view)
    }

    /// Gets or creates a map index with the specified address.
    ///
    /// # Panics
    ///
    /// If the index already exists and is not a map.
    fn ensure_map<I, K, V>(&self, addr: I) -> MapIndex<Self::Base, K, V>
    where
        I: Into<IndexAddress>,
        K: BinaryKey,
        V: BinaryValue,
        Self::Base: RawAccessMut,
    {
        let view = self.get_or_create_view(addr.into(), IndexType::Map);
        MapIndex::new(view)
    }

    /// Gets or creates a Merkelized list index with the specified address.
    ///
    /// # Panics
    ///
    /// If the index already exists and is not a Merkelized list.
    fn ensure_proof_list<I, V>(&self, addr: I) -> ProofListIndex<Self::Base, V>
    where
        I: Into<IndexAddress>,
        V: BinaryValue,
        Self::Base: RawAccessMut,
    {
        let view = self.get_or_create_view(addr.into(), IndexType::ProofList);
        ProofListIndex::new(view)
    }

    /// Gets or creates a Merkelized list index with the specified address.
    ///
    /// # Panics
    ///
    /// If the index already exists and is not a Merkelized list.
    fn ensure_proof_map<I, K, V>(&self, addr: I) -> ProofMapIndex<Self::Base, K, V>
    where
        I: Into<IndexAddress>,
        K: BinaryKey + ObjectHash,
        V: BinaryValue,
        Self::Base: RawAccessMut,
    {
        let view = self.get_or_create_view(addr.into(), IndexType::ProofMap);
        ProofMapIndex::new(view)
    }

    /// Gets or creates a sparse list index with the specified address.
    ///
    /// # Panics
    ///
    /// If the index already exists and is not a sparse list.
    fn ensure_sparse_list<I, V>(&self, addr: I) -> SparseListIndex<Self::Base, V>
    where
        I: Into<IndexAddress>,
        V: BinaryValue,
        Self::Base: RawAccessMut,
    {
        let view = self.get_or_create_view(addr.into(), IndexType::SparseList);
        SparseListIndex::new(view)
    }

    /// Gets or creates a key set index with the specified address.
    ///
    /// # Panics
    ///
    /// If the index already exists and is not a key set.
    fn ensure_key_set<I, V>(&self, addr: I) -> KeySetIndex<Self::Base, V>
    where
        I: Into<IndexAddress>,
        V: BinaryKey,
        Self::Base: RawAccessMut,
    {
        let view = self.get_or_create_view(addr.into(), IndexType::KeySet);
        KeySetIndex::new(view)
    }

    /// Gets or creates a value set index with the specified address.
    ///
    /// # Panics
    ///
    /// If the index already exists and is not a value set.
    fn ensure_value_set<I, V>(&self, addr: I) -> ValueSetIndex<Self::Base, V>
    where
        I: Into<IndexAddress>,
        V: BinaryValue + ObjectHash,
        Self::Base: RawAccessMut,
    {
        let view = self.get_or_create_view(addr.into(), IndexType::ValueSet);
        ValueSetIndex::new(view)
    }

    /// Ensures that the given address corresponds to an index with the specified type
    /// creating the index if necessary.
    ///
    /// # Panics
    ///
    /// If the index already exists and has a different type.
    fn ensure_type<I>(&self, addr: I, index_type: IndexType) -> &Self
    where
        I: Into<IndexAddress>,
        Self::Base: RawAccessMut,
    {
        self.get_or_create_view(addr.into(), index_type);
        self
    }
}

impl<T: RawAccess> Access for T {
    type Base = Self;

    fn get_view(&self, addr: IndexAddress) -> Option<ViewWithMetadata<Self::Base>> {
        ViewWithMetadata::get(self.clone(), &addr)
    }

    fn get_or_create_view(
        &self,
        addr: IndexAddress,
        index_type: IndexType,
    ) -> ViewWithMetadata<Self::Base>
    where
        Self: RawAccessMut,
    {
        ViewWithMetadata::get_or_create(self.clone(), &addr, index_type).unwrap_or_else(|e| {
            panic!(
                "Index at {:?} is expected to be a {:?}, but is really a {:?}",
                addr,
                index_type,
                e.index_type()
            );
        })
    }
}

/// Access that prepends the specified prefix to each created view.
#[derive(Debug, Clone)]
pub struct Prefixed<'a, T> {
    access: T,
    prefix: Cow<'a, str>,
}

impl<'a, T: RawAccess> Prefixed<'a, T> {
    /// Creates new prefixed access.
    ///
    /// # Panics
    ///
    /// Will panic if the prefix does not conform to valid names for indexes.
    pub fn new(prefix: impl Into<Cow<'a, str>>, access: T) -> Self {
        let prefix = prefix.into();
        assert_valid_name(prefix.as_ref());
        Self { access, prefix }
    }
}

impl<T: RawAccess> Access for Prefixed<'_, T> {
    type Base = T;

    fn get_view(&self, addr: IndexAddress) -> Option<ViewWithMetadata<Self::Base>> {
        let prefixed_addr = addr.prepend_name(self.prefix.as_ref());
        self.access.get_view(prefixed_addr)
    }

    fn get_or_create_view(
        &self,
        addr: IndexAddress,
        index_type: IndexType,
    ) -> ViewWithMetadata<Self::Base>
    where
        Self::Base: RawAccessMut,
    {
        let prefixed_addr = addr.prepend_name(self.prefix.as_ref());
        self.access.get_or_create_view(prefixed_addr, index_type)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Database, TemporaryDB};

    #[test]
    fn prefixed_works() {
        let db = TemporaryDB::new();
        let fork = db.fork();
        {
            let prefixed = Prefixed::new("test", &fork);
            let mut list = prefixed.ensure_list::<_, i32>("foo");
            list.extend(vec![1, 2, 3]);
        }
        {
            let list = fork.as_ref().list::<_, i32>("test.foo").unwrap();
            assert_eq!(list.len(), 3);
            assert_eq!(list.iter().collect::<Vec<_>>(), vec![1, 2, 3]);
        }
        db.merge_sync(fork.into_patch()).unwrap();

        let snapshot = db.snapshot();
        let list = snapshot.as_ref().list::<_, i32>("test.foo").unwrap();
        assert_eq!(list.len(), 3);
        assert_eq!(list.iter().collect::<Vec<_>>(), vec![1, 2, 3]);

        let prefixed = Prefixed::new("test", &snapshot);
        let list = prefixed.list::<_, i32>("foo").unwrap();
        assert_eq!(list.len(), 3);
        assert_eq!(list.iter().collect::<Vec<_>>(), vec![1, 2, 3]);
    }

    #[test]
    fn prefixed_views_do_not_collide() {
        let db = TemporaryDB::new();
        let fork = db.fork();
        let foo_space = Prefixed::new("foo", &fork);
        let bar_space = Prefixed::new("bar", &fork);
        {
            let mut list = foo_space.ensure_list("test");
            list.push("Test".to_owned());
            let mut other_list = bar_space.ensure_list("test");
            other_list.extend(vec![1_u64, 2, 3]);

            assert_eq!(list.len(), 1);
            assert_eq!(other_list.len(), 3);
        }
        db.merge_sync(fork.into_patch()).unwrap();

        let snapshot = db.snapshot();
        let foo_space = Prefixed::new("foo", &snapshot);
        let list = foo_space.list::<_, String>("test").unwrap();
        assert_eq!(list.get(0), Some("Test".to_owned()));
        let bar_space = Prefixed::new("bar", &snapshot);
        let list = bar_space.list::<_, u64>("test").unwrap();
        assert_eq!(list.get(0), Some(1_u64));

        // It is possible to create indexes of the different types at the same place.
        let fork = db.fork();
        let foo_space = Prefixed::new("foo", &fork);
        foo_space.ensure_type(("fam", &1_u32), IndexType::List);
        let bar_space = Prefixed::new("bar", &fork);
        bar_space.ensure_type(("fam", &1_u32), IndexType::ProofMap);
        db.merge_sync(fork.into_patch()).unwrap();

        let snapshot = db.snapshot();
        let view = snapshot
            .as_ref()
            .get_view(("foo.fam", &1_u32).into())
            .unwrap();
        assert_eq!(view.index_type(), IndexType::List);
        let view = snapshot
            .as_ref()
            .get_view(("bar.fam", &1_u32).into())
            .unwrap();
        assert_eq!(view.index_type(), IndexType::ProofMap);
    }
}
