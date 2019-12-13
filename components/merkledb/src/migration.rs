//! Migration utilities.
//!
//! # Migration workflow
//!
//! **Migration** in MerkleDB refers to the ability to update data in indexes, remove indexes,
//! change index type, create new indexes, and package these changes in a way that they
//! can be atomically committed or rolled back. Accumulating changes in the migration,
//! on the other hand, can be performed iteratively, including after a process shutdown.
//!
//! Each migration is confined to a *namespace*, defined in the same way as for [`Prefixed`]
//! accesses. For example, namespace `test` concerns indexes with an address starting with
//! `test.`, such as `test.foo` or `(test.bar, 1_u32)`, but not `test` or `test_.foo`.
//!
//! Migration is non-destructive, i.e., does not remove the old versions of migrated indexes.
//! Instead, new indexes are created with a different address of form `^test.foo` (notice
//! the caret char `^`, which can be read as "the next version of"). Indexes `^test.foo` and
//! `test.foo` can peacefully coexist and have separate data and even different types.
//!
//! Retaining an index in the migration is a no op. *Removing* an index is explicit; it needs
//! to be performed via [`create_tombstone`] method. Although tombstones do not contain data,
//! they behave like indexes in other regards. For example, it is impossible to create a tombstone
//! and then create an ordinary index at the same address, or vice versa.
//!
//! Indexes created within a migration are not [aggregated] in the default state hash. Instead,
//! they are placed in a separate namespace, the aggregator and state hash for which can be
//! obtained via respective [`SystemSchema`] methods.
//!
//! To finalize a migration, one needs to call [`Fork::finish_migration`]. This will replace
//! old index data with new, remove indexes marked with tombstones, and return migrated indexes
//! to the default state aggregator.
//!
//! [`Prefixed`]: ../access/struct.Prefixed.html
//! [`create_tombstone`]: ../access/trait.AccessExt.html#method.create_tombstone
//! [aggregated]: ../index.html#state-aggregation
//! [`SystemSchema`]: ../struct.SystemSchema.html
//! [`Fork::finish_migration`]: ../struct.Fork.html#method.finish_migration
//!
//! # Examples
//!
//! ```
//! # use exonum_merkledb::{access::AccessExt, Database, SystemSchema, TemporaryDB};
//! # use exonum_merkledb::migration::Migration;
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
//! let mut migration = Migration::new(Arc::clone(&db) as Arc<dyn Database>, "test");
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
//! let aggregated = SystemSchema::new(&snapshot).namespace_state_aggregator("test");
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
//! assert_eq!(snapshot.get_proof_list::<_, u32>("^test.list").len(), 3);
//!
//! // The migration can be committed as follows.
//! let mut fork = db.fork();
//! fork.finish_migration("test");
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

use crate::{
    access::Prefixed, validation::assert_valid_name_component, Database, Fork, ReadonlyFork,
    SystemSchema,
};

/// Migration helper.
///
/// See the [module docs](index.html) for examples of usage.
pub struct Migration {
    db: Arc<dyn Database>,
    fork: Fork,
    namespace: String,
}

impl fmt::Debug for Migration {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter
            .debug_tuple("Migration")
            .field(&self.namespace)
            .finish()
    }
}

impl Migration {
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
    pub fn new_data(&self) -> Prefixed<'_, &Fork> {
        Prefixed::for_migration(&self.namespace, &self.fork)
    }

    /// Returns readonly access to the old version of migrated data.
    pub fn old_data(&self) -> Prefixed<'_, ReadonlyFork<'_>> {
        Prefixed::new(&self.namespace, self.fork.readonly())
    }

    /// Merges the changes to the migrated data to the database. Returns an error
    /// if the merge has failed.
    pub fn merge(&mut self) -> crate::Result<()> {
        let fork = mem::replace(&mut self.fork, self.db.fork());
        self.db.merge(fork.into_patch())?;
        self.fork = self.db.fork();
        Ok(())
    }

    /// Merges the changes to the migrated data to the database.
    /// Returns hash representing migrated data state, or an error if the merge has failed.
    pub fn finish(self) -> crate::Result<Hash> {
        let patch = self.fork.into_patch();
        let hash = SystemSchema::new(&patch).namespace_state_hash(&self.namespace);
        self.db.merge(patch).map(|()| hash)
    }
}
