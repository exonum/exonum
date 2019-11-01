//! Extension traits to simplify index instantiation.

use crate::{
    views::{IndexType, ViewWithMetadata},
    BinaryKey, BinaryValue, Entry, IndexAccess, IndexAccessMut, IndexAddress, KeySetIndex,
    ListIndex, MapIndex, ObjectHash, ProofListIndex, ProofMapIndex, SparseListIndex, ValueSetIndex,
};

pub trait AccessExt {
    type Base: IndexAccess;

    /// Gets an entry index with the specified address.
    ///
    /// # Panics
    ///
    /// If the index exists, but is not an entry.
    fn entry<I, V>(&self, addr: I) -> Option<Entry<Self::Base, V>>
    where
        I: Into<IndexAddress>,
        V: BinaryValue;

    /// Gets a list index with the specified address.
    ///
    /// # Panics
    ///
    /// If the index exists, but is not a list.
    fn list<I, V>(&self, addr: I) -> Option<ListIndex<Self::Base, V>>
    where
        I: Into<IndexAddress>,
        V: BinaryValue;

    /// Gets a map index with the specified address.
    ///
    /// # Panics
    ///
    /// If the index exists, but is not a map.
    fn map<I, K, V>(&self, addr: I) -> Option<MapIndex<Self::Base, K, V>>
    where
        I: Into<IndexAddress>,
        K: BinaryKey,
        V: BinaryValue;

    /// Gets a Merkelized list index with the specified address.
    ///
    /// # Panics
    ///
    /// If the index exists, but is not a Merkelized list.
    fn proof_list<I, V>(&self, addr: I) -> Option<ProofListIndex<Self::Base, V>>
    where
        I: Into<IndexAddress>,
        V: BinaryValue;

    /// Gets a Merkelized list index with the specified address.
    ///
    /// # Panics
    ///
    /// If the index exists, but is not a Merkelized list.
    fn proof_map<I, K, V>(&self, addr: I) -> Option<ProofMapIndex<Self::Base, K, V>>
    where
        I: Into<IndexAddress>,
        K: BinaryKey + ObjectHash,
        V: BinaryValue;

    /// Gets a sparse list index with the specified address.
    ///
    /// # Panics
    ///
    /// If the index exists, but is not a sparse list.
    fn sparse_list<I, V>(&self, addr: I) -> Option<SparseListIndex<Self::Base, V>>
    where
        I: Into<IndexAddress>,
        V: BinaryValue;

    /// Gets a key set index with the specified address.
    ///
    /// # Panics
    ///
    /// If the index exists, but is not a key set.
    fn key_set<I, V>(&self, addr: I) -> Option<KeySetIndex<Self::Base, V>>
    where
        I: Into<IndexAddress>,
        V: BinaryKey;

    /// Gets a value set index with the specified address.
    ///
    /// # Panics
    ///
    /// If the index exists, but is not a value set.
    fn value_set<I, V>(&self, addr: I) -> Option<ValueSetIndex<Self::Base, V>>
    where
        I: Into<IndexAddress>,
        V: BinaryValue + ObjectHash;

    /// Gets or creates a list index with the specified address.
    ///
    /// # Panics
    ///
    /// If the index already exists and is not a list.
    fn ensure_list<I, V>(&self, addr: I) -> ListIndex<Self::Base, V>
    where
        I: Into<IndexAddress>,
        V: BinaryValue,
        Self::Base: IndexAccessMut;

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
        Self::Base: IndexAccessMut;

    /// Gets or creates a Merkelized list index with the specified address.
    ///
    /// # Panics
    ///
    /// If the index already exists and is not a Merkelized list.
    fn ensure_proof_list<I, V>(&self, addr: I) -> ProofListIndex<Self::Base, V>
    where
        I: Into<IndexAddress>,
        V: BinaryValue,
        Self::Base: IndexAccessMut;

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
        Self::Base: IndexAccessMut;

    /// Gets or creates a sparse list index with the specified address.
    ///
    /// # Panics
    ///
    /// If the index already exists and is not a sparse list.
    fn ensure_sparse_list<I, V>(&self, addr: I) -> SparseListIndex<Self::Base, V>
    where
        I: Into<IndexAddress>,
        V: BinaryValue,
        Self::Base: IndexAccessMut;

    /// Gets or creates a key set index with the specified address.
    ///
    /// # Panics
    ///
    /// If the index already exists and is not a key set.
    fn ensure_key_set<I, V>(&self, addr: I) -> KeySetIndex<Self::Base, V>
    where
        I: Into<IndexAddress>,
        V: BinaryKey,
        Self::Base: IndexAccessMut;

    /// Gets or creates a value set index with the specified address.
    ///
    /// # Panics
    ///
    /// If the index already exists and is not a value set.
    fn ensure_value_set<I, V>(&self, addr: I) -> ValueSetIndex<Self::Base, V>
    where
        I: Into<IndexAddress>,
        V: BinaryValue + ObjectHash,
        Self::Base: IndexAccessMut;
}

fn get_view<T>(base: &T, addr: impl Into<IndexAddress>) -> Option<ViewWithMetadata<T>>
where
    T: IndexAccess,
{
    let addr = addr.into();
    ViewWithMetadata::get(base.clone(), &addr)
}

fn get_or_create_view<T: IndexAccessMut>(
    base: &T,
    addr: impl Into<IndexAddress>,
    index_type: IndexType,
) -> ViewWithMetadata<T> {
    let addr = addr.into();
    ViewWithMetadata::get_or_create(base.clone(), &addr, index_type).unwrap_or_else(|e| {
        panic!(
            "Index at {:?} is expected to be a {:?}, but is really a {:?}",
            addr,
            index_type,
            e.index_type()
        );
    })
}

impl<T: IndexAccess> AccessExt for T {
    type Base = Self;

    fn entry<I, V>(&self, addr: I) -> Option<Entry<Self::Base, V>>
    where
        I: Into<IndexAddress>,
        V: BinaryValue,
    {
        let view = get_view(self, addr)?;
        Some(Entry::new(view))
    }

    fn list<I, V>(&self, addr: I) -> Option<ListIndex<Self::Base, V>>
    where
        I: Into<IndexAddress>,
        V: BinaryValue,
    {
        let view = get_view(self, addr)?;
        Some(ListIndex::new(view))
    }

    fn map<I, K, V>(&self, addr: I) -> Option<MapIndex<Self::Base, K, V>>
    where
        I: Into<IndexAddress>,
        K: BinaryKey,
        V: BinaryValue,
    {
        let view = get_view(self, addr)?;
        Some(MapIndex::new(view))
    }

    fn proof_list<I, V>(&self, addr: I) -> Option<ProofListIndex<Self::Base, V>>
    where
        I: Into<IndexAddress>,
        V: BinaryValue,
    {
        let view = get_view(self, addr)?;
        Some(ProofListIndex::new(view))
    }

    fn proof_map<I, K, V>(&self, addr: I) -> Option<ProofMapIndex<Self::Base, K, V>>
    where
        I: Into<IndexAddress>,
        K: BinaryKey + ObjectHash,
        V: BinaryValue,
    {
        let view = get_view(self, addr)?;
        Some(ProofMapIndex::new(view))
    }

    fn sparse_list<I, V>(&self, addr: I) -> Option<SparseListIndex<Self::Base, V>>
    where
        I: Into<IndexAddress>,
        V: BinaryValue,
    {
        let view = get_view(self, addr)?;
        Some(SparseListIndex::new(view))
    }

    fn key_set<I, V>(&self, addr: I) -> Option<KeySetIndex<Self::Base, V>>
    where
        I: Into<IndexAddress>,
        V: BinaryKey,
    {
        let view = get_view(self, addr)?;
        Some(KeySetIndex::new(view))
    }

    fn value_set<I, V>(&self, addr: I) -> Option<ValueSetIndex<Self::Base, V>>
    where
        I: Into<IndexAddress>,
        V: BinaryValue + ObjectHash,
    {
        let view = get_view(self, addr)?;
        Some(ValueSetIndex::new(view))
    }

    fn ensure_list<I, V>(&self, addr: I) -> ListIndex<Self::Base, V>
    where
        I: Into<IndexAddress>,
        V: BinaryValue,
        Self: IndexAccessMut,
    {
        let view = get_or_create_view(self, addr, IndexType::List);
        ListIndex::new(view)
    }

    fn ensure_map<I, K, V>(&self, addr: I) -> MapIndex<Self::Base, K, V>
    where
        I: Into<IndexAddress>,
        K: BinaryKey,
        V: BinaryValue,
        Self: IndexAccessMut,
    {
        let view = get_or_create_view(self, addr, IndexType::Map);
        MapIndex::new(view)
    }

    fn ensure_proof_list<I, V>(&self, addr: I) -> ProofListIndex<Self::Base, V>
    where
        I: Into<IndexAddress>,
        V: BinaryValue,
        Self: IndexAccessMut,
    {
        let view = get_or_create_view(self, addr, IndexType::ProofList);
        ProofListIndex::new(view)
    }

    fn ensure_proof_map<I, K, V>(&self, addr: I) -> ProofMapIndex<Self::Base, K, V>
    where
        I: Into<IndexAddress>,
        K: BinaryKey + ObjectHash,
        V: BinaryValue,
        Self: IndexAccessMut,
    {
        let view = get_or_create_view(self, addr, IndexType::ProofMap);
        ProofMapIndex::new(view)
    }

    fn ensure_sparse_list<I, V>(&self, addr: I) -> SparseListIndex<Self::Base, V>
    where
        I: Into<IndexAddress>,
        V: BinaryValue,
        Self: IndexAccessMut,
    {
        let view = get_or_create_view(self, addr, IndexType::SparseList);
        SparseListIndex::new(view)
    }

    fn ensure_key_set<I, V>(&self, addr: I) -> KeySetIndex<Self::Base, V>
    where
        I: Into<IndexAddress>,
        V: BinaryKey,
        Self: IndexAccessMut,
    {
        let view = get_or_create_view(self, addr, IndexType::KeySet);
        KeySetIndex::new(view)
    }

    fn ensure_value_set<I, V>(&self, addr: I) -> ValueSetIndex<Self::Base, V>
    where
        I: Into<IndexAddress>,
        V: BinaryValue + ObjectHash,
        Self: IndexAccessMut,
    {
        let view = get_or_create_view(self, addr, IndexType::ValueSet);
        ValueSetIndex::new(view)
    }
}
