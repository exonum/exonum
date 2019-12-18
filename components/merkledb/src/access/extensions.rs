//! Extension traits to simplify index instantiation.

use super::{Access, AccessError, FromAccess};
use crate::{
    proof_map_index::{Raw, ToProofPath},
    views::IndexType,
    BinaryKey, BinaryValue, Entry, Group, IndexAddress, KeySetIndex, ListIndex, MapIndex,
    ObjectHash, ProofEntry, ProofListIndex, ProofMapIndex, SparseListIndex, ValueSetIndex,
};

/// Extension trait allowing for easy access to indices from any type implementing
/// `Access`.
///
/// # Implementation details
///
/// This trait is essentially a thin wrapper around [`FromAccess`]. Where `FromAccess` returns
/// an access error, the methods of this trait will `unwrap()` the error and panic.
///
/// [`FromAccess`]: trait.FromAccess.html
///
/// # Examples
///
/// ```
/// use exonum_merkledb::{access::AccessExt, Database, ListIndex, TemporaryDB};
///
/// let db = TemporaryDB::new();
/// let fork = db.fork();
/// // Extension methods can be used on `Fork`s:
/// {
///     let mut list: ListIndex<_, String> = fork.get_list("list");
///     list.push("foo".to_owned());
/// }
///
/// // ...and on `Snapshot`s:
/// let snapshot = db.snapshot();
/// assert!(snapshot
///     .get_map::<_, u64, String>("map")
///     .get(&0)
///     .is_none());
///
/// // ...and on `ReadonlyFork`s:
/// {
///     let list = fork.readonly().get_list::<_, String>("list");
///     assert_eq!(list.len(), 1);
/// }
///
/// // ...and on `Patch`es:
/// let patch = fork.into_patch();
/// let list = patch.get_list::<_, String>("list");
/// assert_eq!(list.len(), 1);
/// ```
pub trait AccessExt: Access {
    /// Returns a group of indexes. All indexes in the group have the same type.
    /// Indexes are initialized lazily; i.e., no initialization is performed when the group
    /// is created.
    ///
    /// Note that unlike other methods, this one requires address to be a string.
    /// This is to prevent collisions among groups.
    fn get_group<K, I>(self, name: impl Into<String>) -> Group<Self, K, I>
    where
        K: BinaryKey + ?Sized,
        I: FromAccess<Self>,
    {
        Group::from_access(self, IndexAddress::from_root(name))
            .unwrap_or_else(|e| panic!("MerkleDB error: {}", e))
    }

    /// Gets an entry index with the specified address.
    ///
    /// # Panics
    ///
    /// If the index exists, but is not an entry.
    fn get_entry<I, V>(self, addr: I) -> Entry<Self::Base, V>
    where
        I: Into<IndexAddress>,
        V: BinaryValue,
    {
        Entry::from_access(self, addr.into()).unwrap_or_else(|e| panic!("MerkleDB error: {}", e))
    }

    /// Gets a hashed entry index with the specified address.
    ///
    /// # Panics
    ///
    /// If the index exists, but is not a hashed entry.
    fn get_proof_entry<I, V>(self, addr: I) -> ProofEntry<Self::Base, V>
    where
        I: Into<IndexAddress>,
        V: BinaryValue + ObjectHash,
    {
        ProofEntry::from_access(self, addr.into()).unwrap()
    }

    /// Gets a list index with the specified address.
    ///
    /// # Panics
    ///
    /// If the index exists, but is not a list.
    fn get_list<I, V>(self, addr: I) -> ListIndex<Self::Base, V>
    where
        I: Into<IndexAddress>,
        V: BinaryValue,
    {
        ListIndex::from_access(self, addr.into())
            .unwrap_or_else(|e| panic!("MerkleDB error: {}", e))
    }

    /// Gets a map index with the specified address.
    ///
    /// # Panics
    ///
    /// If the index exists, but is not a map.
    fn get_map<I, K, V>(self, addr: I) -> MapIndex<Self::Base, K, V>
    where
        I: Into<IndexAddress>,
        K: BinaryKey + ?Sized,
        V: BinaryValue,
    {
        MapIndex::from_access(self, addr.into()).unwrap_or_else(|e| panic!("MerkleDB error: {}", e))
    }

    /// Gets a Merkelized list index with the specified address.
    ///
    /// # Panics
    ///
    /// If the index exists, but is not a Merkelized list.
    fn get_proof_list<I, V>(self, addr: I) -> ProofListIndex<Self::Base, V>
    where
        I: Into<IndexAddress>,
        V: BinaryValue,
    {
        ProofListIndex::from_access(self, addr.into())
            .unwrap_or_else(|e| panic!("MerkleDB error: {}", e))
    }

    /// Gets a Merkelized map index with the specified address.
    ///
    /// # Panics
    ///
    /// If the index exists, but is not a Merkelized map.
    fn get_proof_map<I, K, V>(self, addr: I) -> ProofMapIndex<Self::Base, K, V>
    where
        I: Into<IndexAddress>,
        K: BinaryKey + ObjectHash,
        V: BinaryValue,
    {
        ProofMapIndex::from_access(self, addr.into())
            .unwrap_or_else(|e| panic!("MerkleDB error: {}", e))
    }

    /// Variant of the proof map with keys that can be mapped directly to `ProofPath`.
    ///
    /// # Panics
    ///
    /// If the index exists, but is not a Merkelized map.
    fn get_raw_proof_map<I, K, V>(self, addr: I) -> ProofMapIndex<Self::Base, K, V, Raw>
    where
        I: Into<IndexAddress>,
        K: BinaryKey,
        V: BinaryValue,
        Raw: ToProofPath<K>,
    {
        ProofMapIndex::<_, _, _, Raw>::from_access(self, addr.into())
            .unwrap_or_else(|e| panic!("MerkleDB error: {}", e))
    }

    /// Gets a generic proof map. Requires explicit `KeyMode` to be specified.
    ///
    /// # Examples
    ///
    /// ```
    /// # use exonum_merkledb::{
    /// #     access::AccessExt, Fork, Database, ListIndex, TemporaryDB, ProofMapIndex,
    /// #     RawProofMapIndex,
    /// # };
    /// # use exonum_crypto::PublicKey;
    /// let db = TemporaryDB::new();
    /// let fork = db.fork();
    /// // Hashed variant for keys implementing `ObjectHash`.
    /// let hashed_map: ProofMapIndex<_, u32, u32> = fork.get_generic_proof_map("hashed");
    /// // Raw variant for keys that can be mapped directly to `ProofPath`.
    /// let raw_map: RawProofMapIndex<_, PublicKey, u32> = fork.get_generic_proof_map("raw");
    /// ```
    ///
    /// # Panics
    ///
    /// If the index exists, but is not a Merkelized map.
    fn get_generic_proof_map<I, K, V, KeyMode>(
        self,
        addr: I,
    ) -> ProofMapIndex<Self::Base, K, V, KeyMode>
    where
        I: Into<IndexAddress>,
        K: BinaryKey,
        V: BinaryValue,
        KeyMode: ToProofPath<K>,
    {
        ProofMapIndex::<_, _, _, KeyMode>::from_access(self, addr.into())
            .unwrap_or_else(|e| panic!("MerkleDB error: {}", e))
    }

    /// Gets a sparse list index with the specified address.
    ///
    /// # Panics
    ///
    /// If the index exists, but is not a sparse list.
    fn get_sparse_list<I, V>(self, addr: I) -> SparseListIndex<Self::Base, V>
    where
        I: Into<IndexAddress>,
        V: BinaryValue,
    {
        SparseListIndex::from_access(self, addr.into())
            .unwrap_or_else(|e| panic!("MerkleDB error: {}", e))
    }

    /// Gets a key set index with the specified address.
    ///
    /// # Panics
    ///
    /// If the index exists, but is not a key set.
    fn get_key_set<I, V>(self, addr: I) -> KeySetIndex<Self::Base, V>
    where
        I: Into<IndexAddress>,
        V: BinaryKey,
    {
        KeySetIndex::from_access(self, addr.into())
            .unwrap_or_else(|e| panic!("MerkleDB error: {}", e))
    }

    /// Gets a value set index with the specified address.
    ///
    /// # Panics
    ///
    /// If the index exists, but is not a value set.
    fn get_value_set<I, V>(self, addr: I) -> ValueSetIndex<Self::Base, V>
    where
        I: Into<IndexAddress>,
        V: BinaryValue + ObjectHash,
    {
        ValueSetIndex::from_access(self, addr.into())
            .unwrap_or_else(|e| panic!("MerkleDB error: {}", e))
    }

    /// Touches an index at the specified address, asserting that it has a specific type.
    ///
    /// # Errors
    ///
    /// Returns an error if the index has in invalid type.
    fn touch_index<I>(self, addr: I, index_type: IndexType) -> Result<(), AccessError>
    where
        I: Into<IndexAddress>,
    {
        self.get_or_create_view(addr.into(), index_type).map(drop)
    }
}

impl<T: Access> AccessExt for T {}
