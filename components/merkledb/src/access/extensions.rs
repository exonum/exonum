//! Extension traits to simplify index instantiation.

use super::{Access, Ensure, RawAccessMut, Restore};
use crate::{
    views::IndexType, BinaryKey, BinaryValue, Entry, Group, IndexAddress, KeySetIndex, ListIndex,
    MapIndex, ObjectHash, ProofListIndex, ProofMapIndex, SparseListIndex, ValueSetIndex,
};

/// Extension trait allowing for easy access to indices from any type implementing
/// `Access`.
pub trait AccessExt: Access {
    /// Returns a group of indexes. All indexes in the group have the same type.
    /// Indexes are initialized lazily; i.e., no initialization is performed when the group
    /// is created.
    ///
    /// Note that unlike other methods, this one requires address to be a string.
    /// This is to prevent collisions among groups.
    fn group<K, I>(&self, name: impl Into<String>) -> Group<Self, K, I>
    where
        K: BinaryKey + ?Sized,
        I: Restore<Self>,
    {
        // We know that `Group` implementation of `Restore` never fails
        Group::restore(self, IndexAddress::with_root(name)).unwrap()
    }

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
        Entry::restore(self, addr.into()).ok()
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
        ListIndex::restore(self, addr.into()).ok()
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
        MapIndex::restore(self, addr.into()).ok()
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
        ProofListIndex::restore(self, addr.into()).ok()
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
        ProofMapIndex::restore(self, addr.into()).ok()
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
        SparseListIndex::restore(self, addr.into()).ok()
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
        KeySetIndex::restore(self, addr.into()).ok()
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
        ValueSetIndex::restore(self, addr.into()).ok()
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
        Entry::ensure(self, addr.into()).unwrap()
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
        ListIndex::ensure(self, addr.into()).unwrap()
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
        MapIndex::ensure(self, addr.into()).unwrap()
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
        ProofListIndex::ensure(self, addr.into()).unwrap()
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
        ProofMapIndex::ensure(self, addr.into()).unwrap()
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
        SparseListIndex::ensure(self, addr.into()).unwrap()
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
        KeySetIndex::ensure(self, addr.into()).unwrap()
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
        ValueSetIndex::ensure(self, addr.into()).unwrap()
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
        self.get_or_create_view(addr.into(), index_type).unwrap();
        self
    }
}

impl<T: Access> AccessExt for T {}
