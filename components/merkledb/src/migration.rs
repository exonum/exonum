//! Migration utilities.
//!
//! # Migration workflow
//!
//! **Migration** refers to the ability to update data in indexes, remove indexes,
//! change index type, create new indexes, and package these changes in a way that they
//! can be atomically committed or rolled back. Accumulating changes in the migration,
//! on the other hand, can be performed iteratively, including after a process shutdown.
//!
//! Each migration is confined to a *namespace*, defined in a similar way as [`Prefixed`]
//! accesses. For example, namespace `test` concerns indexes with an address starting with
//! `test.`, such as `test.foo` or `(test.bar, 1_u32)`, but not `test` or `test_.foo`.
//! The namespace can be accessed via [`Migration`].
//!
//! Migration is non-destructive, i.e., does not remove the old versions of migrated indexes.
//! Instead, new indexes are created in a separate namespace. For example, index `foo`
//! in the migration namespace `test` and the original `test.foo` index can peacefully coexist
//! and have separate data and even different types. The movement of data is performed only
//! when the migration is finalized.
//!
//! Retaining an index in the migration is a no op. *Removing* an index is explicit; it needs
//! to be performed via [`create_tombstone`] method. Although tombstones do not contain data,
//! they behave like indexes in other regards. For example, it is impossible to create a tombstone
//! and then create an ordinary index at the same address, or vice versa.
//!
//! Indexes created within a migration are not [aggregated] in the default state hash. Instead,
//! they are placed in a separate namespace, the aggregator and state hash for which can be
//! obtained via respective [`Migration`] methods.
//!
//! It is possible to periodically persist migrated data to the database
//! (indeed, this is a best practice to avoid out-of-memory errors). It is even possible
//! to restart the process handling the migration, provided it can recover from such a restart
//! on the application level.
//!
//! # Finalizing Migration
//!
//! To finalize a migration, one needs to call [`Fork::flush_migration`]. This will replace
//! old index data with new, remove indexes marked with tombstones, and return migrated indexes
//! to the default state aggregator. To roll back a migration, use [`Fork::rollback_migration`].
//! This will remove the new index data and corresponding metadata.
//!
//! [`Migration`]: struct.Migration.html
//! [`Prefixed`]: ../access/struct.Prefixed.html
//! [`create_tombstone`]: struct.Migration.html#method.create_tombstone
//! [aggregated]: ../index.html#state-aggregation
//! [`Fork::flush_migration`]: ../struct.Fork.html#method.flush_migration
//! [`Fork::rollback_migration`]: ../struct.Fork.html#method.rollback_migration
//!
//! # Examples
//!
//! ```
//! # use exonum_merkledb::{access::AccessExt, Database, SystemSchema, TemporaryDB};
//! # use exonum_merkledb::migration::{Migration, MigrationHelper};
//! # use std::sync::Arc;
//! # fn main() -> exonum_merkledb::Result<()> {
//! let db = Arc::new(TemporaryDB::new());
//! // Create initial data in the database.
//! let fork = db.fork();
//! fork.get_list("test.list").extend(vec![1_u32, 2, 3]);
//! fork.get_proof_entry("test.entry").set("text".to_owned());
//! fork.get_map(("test.group", &0_u8)).put(&1, 2);
//! fork.get_map(("test.group", &1_u8)).put(&3, 4);
//! db.merge(fork.into_patch())?;
//! let initial_state_hash = SystemSchema::new(&db.snapshot()).state_hash();
//!
//! // Create migration helper.
//! let mut migration = MigrationHelper::new(Arc::clone(&db) as Arc<dyn Database>, "test");
//! {
//!     // Merkelize the data in the list.
//!     let old_list = migration.old_data().get_list::<_, u32>("list");
//!     let new_data = migration.new_data();
//!     new_data.get_proof_list("list").extend(&old_list);
//! }
//!
//! // It is possible to merge incomplete changes to the DB.
//! migration.merge()?;
//! // Changes in the migrated data do not influence the default state hash.
//! let snapshot = db.snapshot();
//! let intermediate_state_hash = SystemSchema::new(&snapshot).state_hash();
//! assert_eq!(intermediate_state_hash, initial_state_hash);
//! // Instead, they influence the state hash for the migration namespace
//! // (i.e., `test` in this case).
//! let aggregated = Migration::new("test", &snapshot).state_aggregator();
//! assert!(aggregated.contains("test.list"));
//! assert!(!aggregated.contains("test.entry"));
//!
//! // Leave `test.entry` in place (this is no op).
//! // Remove one of indexes in `test.group`.
//! migration.new_data().create_tombstone(("group", &0_u8));
//! // Create a new index.
//! migration.new_data().get_proof_entry("other_entry").set("other".to_owned());
//!
//! // Finish the migration logic.
//! let migration_hash = migration.finish()?;
//! // For now, migrated and original data co-exist in the storage.
//! let snapshot = db.snapshot();
//! assert_eq!(snapshot.get_list::<_, u32>("test.list").len(), 3);
//! let migration = Migration::new("test", &snapshot);
//! assert_eq!(migration.get_proof_list::<_, u32>("list").len(), 3);
//!
//! // The migration can be committed as follows.
//! let mut fork = db.fork();
//! fork.flush_migration("test");
//! db.merge(fork.into_patch())?;
//! let snapshot = db.snapshot();
//! assert_eq!(snapshot.get_proof_list::<_, u32>("test.list").len(), 3);
//! assert_eq!(
//!     snapshot.get_proof_entry::<_, String>("test.other_entry").get().unwrap(),
//!     "other"
//! );
//! # Ok(())
//! # }
//! ```

use exonum_crypto::Hash;

use std::{fmt, mem, sync::Arc};

use crate::views::IndexMetadata;
use crate::{
    access::{Access, AccessError, Prefixed, RawAccess},
    validation::assert_valid_name_component,
    views::{
        get_state_aggregator, AsReadonly, IndexAddress, IndexType, RawAccessMut, ViewWithMetadata,
    },
    Database, Fork, ObjectHash, ProofMapIndex, ReadonlyFork,
};

/// Access to migrated indexes.
///
/// `Migration` is conceptually similar to a [`Prefixed`] access. For example, an index with
/// address `"list"` in a migration `Migration::new("foo", _)` will map to the address `"foo.list"`
/// after the migration is flushed. The major difference with `Prefixed` is that the indexes
/// in a migration cannot be accessed in any other way. That is, it is impossible to access
/// an index in a migration without constructing a `Migration` object first.
///
/// [`Prefixed`]: ../access/struct.Prefixed.html
#[derive(Debug, Clone, Copy)]
pub struct Migration<'a, T> {
    access: T,
    namespace: &'a str,
}

impl<'a, T: RawAccess> Migration<'a, T> {
    /// Creates a migration in the specified namespace.
    pub fn new(namespace: &'a str, access: T) -> Self {
        Self { namespace, access }
    }

    /// Returns the state hash of indexes within the migration. The state hash is up to date
    /// for `Snapshot`s (including `Patch`es), but is generally stale for `Fork`s.
    pub fn state_hash(&self) -> Hash {
        get_state_aggregator(self.access.clone(), self.namespace).object_hash()
    }
}

impl<T: RawAccess + AsReadonly> Migration<'_, T> {
    /// Returns the state aggregator for the indexes within the migration. The aggregator
    /// is up to date for `Snapshot`s (including `Patch`es), but is generally stale for `Fork`s.
    ///
    /// Note that keys in the aggregator are *full* addresses, which include the migration namespace,
    /// as is the case for the [default aggregator].
    ///
    /// [default aggregator]: ../struct.SystemSchema.html#method.state_aggregator
    ///
    /// # Examples
    ///
    /// ```
    /// # use exonum_merkledb::{access::AccessExt, migration::Migration, Database, TemporaryDB};
    /// let db = TemporaryDB::new();
    /// let fork = db.fork();
    /// {
    ///     let migration = Migration::new("migration", &fork);
    ///     migration.get_proof_entry("entry").set(42);
    /// }
    /// let patch = fork.into_patch();
    /// let migration_view = Migration::new("migration", &patch);
    /// let aggregator = migration_view.state_aggregator();
    /// assert!(aggregator.contains("migration.entry")); // Not `entry`!
    /// ```
    pub fn state_aggregator(&self) -> ProofMapIndex<T::Readonly, str, Hash> {
        get_state_aggregator(self.access.as_readonly(), self.namespace)
    }
}

impl<T: RawAccessMut> Migration<'_, T> {
    /// Marks an index with the specified address as removed during migration.
    ///
    /// # Panics
    ///
    /// Panics if an index already exists at the specified address.
    pub fn create_tombstone<I>(self, addr: I)
    where
        I: Into<IndexAddress>,
    {
        self.get_or_create_view(addr.into(), IndexType::Tombstone)
            .unwrap_or_else(|e| panic!("MerkleDB error: {}", e));
    }
}

impl<T: RawAccess> Access for Migration<'_, T> {
    type Base = T;

    fn get_index_metadata(self, addr: IndexAddress) -> Result<Option<IndexMetadata>, AccessError> {
        let mut prefixed_addr = addr.prepend_name(self.namespace.as_ref());
        prefixed_addr.set_in_migration();
        self.access.get_index_metadata(prefixed_addr)
    }

    fn get_or_create_view(
        self,
        addr: IndexAddress,
        index_type: IndexType,
    ) -> Result<ViewWithMetadata<Self::Base>, AccessError> {
        let mut prefixed_addr = addr.prepend_name(self.namespace.as_ref());
        prefixed_addr.set_in_migration();
        self.access.get_or_create_view(prefixed_addr, index_type)
    }
}

/// Migration helper.
///
/// See the [module docs](index.html) for examples of usage.
pub struct MigrationHelper {
    db: Arc<dyn Database>,
    fork: Fork,
    namespace: String,
}

impl fmt::Debug for MigrationHelper {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter
            .debug_tuple("MigrationHelper")
            .field(&self.namespace)
            .finish()
    }
}

impl MigrationHelper {
    /// Creates a new helper.
    pub fn new(db: impl Into<Arc<dyn Database>>, namespace: &str) -> Self {
        assert_valid_name_component(namespace);

        let db = db.into();
        let fork = db.fork();
        Self {
            db,
            fork,
            namespace: namespace.to_owned(),
        }
    }

    /// Returns full access to the new version of migrated data.
    pub fn new_data(&self) -> Migration<'_, &Fork> {
        Migration::new(&self.namespace, &self.fork)
    }

    /// Returns readonly access to the old version of migrated data.
    pub fn old_data(&self) -> Prefixed<'_, ReadonlyFork<'_>> {
        Prefixed::new(&self.namespace, self.fork.readonly())
    }

    /// Merges the changes to the migrated data to the database. Returns an error
    /// if the merge has failed.
    ///
    /// `merge` does not flush the migration; the migrated data remains in a separate namespace.
    /// Use [`Fork::flush_migration`] to flush the migrated data.
    ///
    /// [`Fork::flush_migration`]: ../struct.Fork.html#method.flush_migration
    pub fn merge(&mut self) -> crate::Result<()> {
        let fork = mem::replace(&mut self.fork, self.db.fork());
        self.db.merge(fork.into_patch())?;
        self.fork = self.db.fork();
        Ok(())
    }

    /// Merges the changes to the migrated data to the database.
    /// Returns hash representing migrated data state, or an error if the merge has failed.
    ///
    /// `finish` does not flush the migration; the migrated data remains in a separate namespace.
    /// Use [`Fork::flush_migration`] to flush the migrated data.
    ///
    /// [`Fork::flush_migration`]: ../struct.Fork.html#method.flush_migration
    pub fn finish(self) -> crate::Result<Hash> {
        let patch = self.fork.into_patch();
        let hash = Migration::new(&self.namespace, &patch).state_hash();
        self.db.merge(patch).map(|()| hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        access::{AccessExt, RawAccess},
        HashTag, ObjectHash, SystemSchema, TemporaryDB,
    };

    #[test]
    fn in_memory_migration() {
        fn check_indexes<T: RawAccess + Copy>(view: T) {
            let list = view.get_proof_list::<_, u64>("name.list");
            assert_eq!(list.len(), 2);
            assert_eq!(list.get(0), Some(4));
            assert_eq!(list.get(1), Some(5));
            assert_eq!(list.get(2), None);
            assert_eq!(list.iter().collect::<Vec<_>>(), vec![4, 5]);

            let map = view.get_map::<_, u64, i32>("name.map");
            assert_eq!(map.get(&1), Some(42));

            let set = view.get_key_set::<_, u8>("name.new");
            assert!(set.contains(&0));
            assert!(!set.contains(&1));

            let list = view.get_proof_list::<_, u64>("name.untouched");
            assert_eq!(list.len(), 2);
            assert_eq!(list.get(0), Some(77));
            assert_eq!(list.iter().collect::<Vec<_>>(), vec![77, 88]);

            assert_eq!(view.get_entry("unrelated").get(), Some(1_u64));
            assert_eq!(view.get_entry("name1.unrelated").get(), Some(2_u64));
            let set = view.get_value_set::<_, String>("name.removed");
            assert_eq!(set.iter().count(), 0);
        }

        let db = TemporaryDB::new();
        let mut fork = db.fork();

        fork.get_list("name.list").extend(vec![1_u32, 2, 3]);
        fork.get_map("name.map").put(&1_u64, "!".to_owned());
        fork.get_value_set("name.removed").insert("!!!".to_owned());
        fork.get_proof_list("name.untouched")
            .extend(vec![77_u64, 88]);
        fork.get_entry("unrelated").set(1_u64);
        fork.get_entry("name1.unrelated").set(2_u64);

        // Start migration.
        let migration = Migration::new("name", &fork);
        migration.get_proof_list("list").extend(vec![4_u64, 5]);
        migration.get_map("map").put(&1_u64, 42_i32);
        migration.get_key_set("new").insert(0_u8);
        migration.create_tombstone("removed");

        fork.flush_migration("name");

        check_indexes(&fork);
        // The newly migrated indexes are emptied.
        let migration = Migration::new("name", &fork);
        assert!(migration.get_proof_list::<_, u64>("list").is_empty());

        // Merge the fork and run the checks again.
        db.merge(fork.into_patch()).unwrap();
        let snapshot = db.snapshot();
        check_indexes(&snapshot);
    }

    #[test]
    fn migration_with_merges() {
        fn check_indexes<T: RawAccess + Copy>(view: T) {
            let list = view.get_proof_list::<_, u64>("name.list");
            assert_eq!(list.len(), 4);
            assert_eq!(list.get(2), Some(6));
            assert_eq!(list.iter_from(1).collect::<Vec<_>>(), vec![5, 6, 7]);

            let map = view.get_map::<_, u64, i32>("name.map");
            assert_eq!(map.get(&1), None);
            assert_eq!(map.get(&2), Some(21));
            assert_eq!(map.get(&3), Some(7));
            assert_eq!(map.keys().collect::<Vec<_>>(), vec![2, 3]);

            // This entry should be removed.
            let entry = view.get_entry::<_, String>(("name.family", &1_u8));
            assert!(!entry.exists());
            // ...but this one should be retained.
            let entry = view.get_entry::<_, String>(("name.family", &2_u8));
            assert_eq!(entry.get().unwrap(), "!!");

            let entry = view.get_proof_entry::<_, String>(("name.untouched", &2_u32));
            assert_eq!(entry.get().unwrap(), "??");

            assert_eq!(view.get_entry("unrelated").get(), Some(1_u64));
            assert_eq!(view.get_entry("name1.unrelated").get(), Some(2_u64));
            let set = view.get_value_set::<_, String>("name.removed");
            assert_eq!(set.iter().count(), 0);
        }

        let db = TemporaryDB::new();

        let fork = db.fork();
        fork.get_list("name.list").extend(vec![1_u32, 2, 3]);
        fork.get_map("name.map").put(&1_u64, "!".to_owned());
        fork.get_entry(("name.family", &1_u8)).set("!".to_owned());
        fork.get_entry(("name.family", &2_u8)).set("!!".to_owned());
        fork.get_proof_entry(("name.untouched", &2_u32))
            .set("??".to_owned());
        fork.get_entry("unrelated").set(1_u64);
        fork.get_entry("name1.unrelated").set(2_u64);
        db.merge(fork.into_patch()).unwrap();

        let fork = db.fork();
        let migration = Migration::new("name", &fork);
        migration.get_proof_list("list").extend(vec![4_u64, 5]);
        migration.get_map("map").put(&1_u64, 42_i32);
        migration.get_key_set("new").insert(0_u8);
        migration.create_tombstone(("name.family", &3_u8));
        // ^-- Removing non-existing indexes is weird, but should work fine.
        db.merge(fork.into_patch()).unwrap();

        let mut fork = db.fork();
        {
            let migration = Migration::new("name", &fork);
            let mut list = migration.get_proof_list::<_, u64>("list");
            assert_eq!(list.len(), 2);
            list.push(6);
            list.push(7);
            assert_eq!(list.len(), 4);

            let mut map = migration.get_map::<_, u64, i32>("map");
            map.clear();
            map.put(&2, 21);
            map.put(&3, 7);

            migration.create_tombstone(("family", &1_u8));
        }
        fork.flush_migration("name");

        check_indexes(&fork);
        db.merge(fork.into_patch()).unwrap();
        let snapshot = db.snapshot();
        check_indexes(&snapshot);
    }

    #[test]
    fn aggregation_within_migrations() {
        let db = TemporaryDB::new();
        let fork = db.fork();
        fork.get_proof_list("name.list").push(1_u64);
        fork.get_proof_list("name.other_list")
            .extend(vec![1_u64, 2, 3]);
        fork.get_proof_entry("name.entry").set("!".to_owned());
        db.merge(fork.into_patch()).unwrap();
        let state_hash = SystemSchema::new(&db.snapshot()).state_hash();

        let fork = db.fork();
        let migration = Migration::new("name", &fork);
        migration.get_proof_list("list").extend(vec![2_u64, 3, 4]);
        migration.get_proof_entry("entry").set("?".to_owned());
        migration.get_proof_entry("new").set("??".to_owned());
        db.merge(fork.into_patch()).unwrap();

        let snapshot = db.snapshot();
        let new_state_hash = SystemSchema::new(&snapshot).state_hash();
        assert_eq!(state_hash, new_state_hash);

        let migration = Migration::new("name", &snapshot);
        let ns_hash = migration.state_hash();
        assert_ne!(ns_hash, HashTag::empty_map_hash());
        let ns_aggregator = migration.state_aggregator();
        assert_eq!(ns_hash, ns_aggregator.object_hash());
        assert_eq!(
            ns_aggregator.keys().collect::<Vec<_>>(),
            vec!["name.entry", "name.list", "name.new"]
        );

        let list = migration.get_proof_list::<_, u64>("list");
        assert_eq!(ns_aggregator.get("name.list"), Some(list.object_hash()));
        let entry = migration.get_proof_entry::<_, String>("entry");
        assert_eq!(ns_aggregator.get("name.entry"), Some(entry.object_hash()));
    }

    #[test]
    fn aggregation_after_migrations() {
        let db = TemporaryDB::new();
        let fork = db.fork();

        fork.get_proof_list("name.list").push(1_u64);
        fork.get_proof_list("name.other_list")
            .extend(vec![1_u64, 2, 3]);
        fork.get_entry("name.entry").set("!".to_owned());
        let other_entry_hash = {
            let mut entry = fork.get_proof_entry("name.other_entry");
            entry.set("!!".to_owned());
            entry.object_hash()
        };

        // Migration.
        let migration = Migration::new("name", &fork);
        migration.get_proof_list("list").extend(vec![2_u64, 3, 4]);
        migration.get_proof_entry("entry").set("?".to_owned());
        let modified_entry_hash = migration
            .get_proof_entry::<_, String>("entry")
            .object_hash();
        migration.get_proof_entry("new").set("??".to_owned());
        let new_entry_hash = migration.get_proof_entry::<_, String>("new").object_hash();
        db.merge(fork.into_patch()).unwrap();

        let mut fork = db.fork();
        Migration::new("name", &fork).create_tombstone("other_list");
        fork.flush_migration("name");

        let patch = fork.into_patch();
        assert_eq!(
            Migration::new("name", &patch).state_hash(),
            HashTag::empty_map_hash()
        );
        let system_schema = SystemSchema::new(&patch);
        let aggregator = system_schema.state_aggregator();
        assert_eq!(
            aggregator.keys().collect::<Vec<_>>(),
            vec!["name.entry", "name.list", "name.new", "name.other_entry"]
        );
        assert_eq!(aggregator.get("name.entry"), Some(modified_entry_hash));
        assert_eq!(aggregator.get("name.new"), Some(new_entry_hash));
        assert_eq!(aggregator.get("name.other_entry"), Some(other_entry_hash));
    }

    #[test]
    fn index_metadata_is_removed() {
        let db = TemporaryDB::new();
        let mut fork = db.fork();

        fork.get_entry("test.foo").set(1_u8);
        Migration::new("test", &fork).create_tombstone("foo");
        fork.flush_migration("test");
        let patch = fork.into_patch();
        assert_eq!(
            patch.get_proof_entry::<_, u8>("test.foo").object_hash(),
            Hash::zero()
        );
    }

    fn test_migration_rollback(with_merge: bool) {
        let db = TemporaryDB::new();
        let mut fork = db.fork();

        fork.get_entry("test.foo").set(1_u8);
        fork.get_proof_list(("test.list", &1))
            .extend(vec![1_i32, 2, 3]);
        let migration = Migration::new("test", &fork);
        migration.get_proof_entry("foo").set(2_u8);
        migration.create_tombstone(("list", &1));
        migration.get_value_set("new").insert("test".to_owned());

        if with_merge {
            db.merge(fork.into_patch()).unwrap();
            fork = db.fork();
        }
        fork.rollback_migration("test");
        assert_eq!(fork.get_entry::<_, u8>("test.foo").get(), Some(1));
        let patch = fork.into_patch();
        assert_eq!(patch.get_entry::<_, u8>("test.foo").get(), Some(1));
        assert_eq!(
            patch
                .get_proof_list::<_, i32>(("test.list", &1))
                .iter()
                .collect::<Vec<_>>(),
            vec![1_i32, 2, 3]
        );

        let migration = Migration::new("test", &patch);
        assert!(!migration.get_proof_entry::<_, u8>("foo").exists());
        // Since migrated indexes don't exist, it should be OK to assign new types to them.
        assert!(!migration.get_entry::<_, ()>(("list", &1)).exists());
        assert!(!migration.get_entry::<_, ()>("new").exists());
    }

    #[test]
    fn in_memory_migration_rollback() {
        test_migration_rollback(false);
    }

    #[test]
    fn migration_rollback_with_merge() {
        test_migration_rollback(true);
    }
}
