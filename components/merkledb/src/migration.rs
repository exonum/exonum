//! Migration utilities.
//!
//! # Stability
//!
//! The entirety of this module is considered unstable. While the supported functionality
//! is unlikely to break, the implementation details may change in the following releases.
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
//! A migration can also store temporary data in a [`Scratchpad`]. This data will be removed
//! when the migration is finalized.
//!
//! Indexes created within a migration are not [aggregated] in the default state hash. Instead,
//! they are placed in a separate namespace, the aggregator and state hash for which can be
//! obtained via respective [`Migration`] methods.
//!
//! It is possible to periodically persist migrated data to the database
//! (indeed, this is a best practice to avoid out-of-memory errors). It is even possible
//! to restart the process handling the migration, provided it can recover from such a restart
//! on the application level. To assist with fault tolerance, use [persistent iterators].
//!
//! # Finalizing Migration
//!
//! To finalize a migration, one needs to call [`flush_migration`]. This will replace
//! old index data with new, remove indexes marked with tombstones, and return migrated indexes
//! to the default state aggregator. To roll back a migration,
//! use [`rollback_migration`]. This will remove the new index data and corresponding metadata.
//! Both `flush_migration` and `rollback_migration` will remove the `Scratchpad` associated
//! with the migration.
//!
//! [`Migration`]: struct.Migration.html
//! [`Prefixed`]: ../access/struct.Prefixed.html
//! [`create_tombstone`]: struct.Migration.html#method.create_tombstone
//! [`Scratchpad`]: struct.Scratchpad.html
//! [aggregated]: ../index.html#state-aggregation
//! [persistent iterators]: struct.PersistentIter.html
//! [`flush_migration`]: fn.flush_migration.html
//! [`rollback_migration`]: fn.rollback_migration.html
//!
//! # Examples
//!
//! ```
//! # use exonum_merkledb::{access::{AccessExt, CopyAccessExt}, Database, SystemSchema, TemporaryDB};
//! # use exonum_merkledb::migration::{flush_migration, Migration, MigrationHelper};
//! # use std::sync::Arc;
//! # fn main() -> Result<(), failure::Error> {
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
//! flush_migration(&mut fork, "test");
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

pub use self::persistent_iter::{PersistentIter, PersistentIters, PersistentKeys};

use exonum_crypto::Hash;
use failure::Fail;

use std::{
    fmt,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use crate::validation::check_index_valid_full_name;
use crate::views::IndexMetadata;
use crate::{
    access::{Access, AccessError, Prefixed, RawAccess},
    validation::assert_valid_name_component,
    views::{
        get_state_aggregator, AsReadonly, GroupKeys, IndexAddress, IndexType, IndexesPool,
        RawAccessMut, View, ViewWithMetadata,
    },
    BinaryKey, Database, Fork, ObjectHash, ProofMapIndex, ReadonlyFork,
};

mod persistent_iter;

/// Name of the column family used to store `Scratchpad`s.
const SCRATCHPAD_NAME: &str = "__scratchpad__";

/// Access to migrated indexes.
///
/// `Migration` is conceptually similar to a [`Prefixed`] access. For example, an index with
/// address `"list"` in a migration `Migration::new("foo", _)` will map to the address `"foo.list"`
/// after the migration is flushed. The major difference with `Prefixed` is that the indexes
/// in a migration cannot be accessed in any other way. That is, it is impossible to access
/// an index in a migration without constructing a `Migration` object first.
///
/// [`Prefixed`]: ../access/struct.Prefixed.html
#[derive(Debug, Clone)]
pub struct Migration<T> {
    access: T,
    namespace: String,
}

// **NB.** Must not be made public! This would allow the caller to violate access restrictions
// imposed by `Migration`.
impl<T> Migration<T> {
    pub(crate) fn access(&self) -> &T {
        &self.access
    }

    pub(crate) fn into_parts(self) -> (String, T) {
        (self.namespace, self.access)
    }
}

impl<T: RawAccess> Migration<T> {
    /// Creates a migration in the specified namespace.
    pub fn new(namespace: impl Into<String>, access: T) -> Self {
        Self {
            namespace: namespace.into(),
            access,
        }
    }

    /// Returns the state hash of indexes within the migration. The state hash is up to date
    /// for `Snapshot`s (including `Patch`es), but is generally stale for `Fork`s.
    pub fn state_hash(&self) -> Hash {
        get_state_aggregator(self.access.clone(), &self.namespace).object_hash()
    }
}

impl<T: RawAccess + AsReadonly> Migration<T> {
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
    /// assert!(aggregator.contains("migration.entry")); // Not "entry": `state_aggregator` uses
    ///                                                  // full index names as keys.
    /// ```
    pub fn state_aggregator(&self) -> ProofMapIndex<T::Readonly, str, Hash> {
        get_state_aggregator(self.access.as_readonly(), &self.namespace)
    }
}

impl<T: RawAccessMut> Migration<T> {
    /// Marks an index with the specified address as removed during migration.
    ///
    /// # Panics
    ///
    /// Panics if an index already exists at the specified address.
    pub fn create_tombstone<I>(&self, addr: I)
    where
        I: Into<IndexAddress>,
    {
        self.clone()
            .get_or_create_view(addr.into(), IndexType::Tombstone)
            .unwrap_or_else(|e| panic!("MerkleDB error: {}", e));
    }
}

impl<T: RawAccess> Access for Migration<T> {
    type Base = T;

    fn get_index_metadata(self, addr: IndexAddress) -> Result<Option<IndexMetadata>, AccessError> {
        let mut prefixed_addr = addr.prepend_name(&self.namespace);
        prefixed_addr.set_in_migration();
        self.access.get_index_metadata(prefixed_addr)
    }

    fn get_or_create_view(
        self,
        addr: IndexAddress,
        index_type: IndexType,
    ) -> Result<ViewWithMetadata<Self::Base>, AccessError> {
        let mut prefixed_addr = addr.prepend_name(&self.namespace);
        prefixed_addr.set_in_migration();
        self.access.get_or_create_view(prefixed_addr, index_type)
    }

    fn group_keys<K>(self, base_addr: IndexAddress) -> GroupKeys<Self::Base, K>
    where
        K: BinaryKey + ?Sized,
        Self::Base: AsReadonly<Readonly = Self::Base>,
    {
        let mut prefixed_addr = base_addr.prepend_name(&self.namespace);
        prefixed_addr.set_in_migration();
        self.access.group_keys(prefixed_addr)
    }
}

/// Access to temporary data that can be used during migration. The scratchpad is cleared
/// at the end of the migration, regardless of whether the migration is successful.
///
/// Like `Migration`s, `Scratchpad`s are separated via namespaces. Scratchpads are optimized
/// for small amounts of data per index. Indexes in a `Scratchpad` are not aggregated into
/// the overall database state or the migration state.
#[derive(Debug, Clone)]
pub struct Scratchpad<T> {
    access: T,
    namespace: String,
}

// **NB.** Must not be made public! This would allow the caller to violate access restrictions
// imposed by `Scratchpad`.
impl<T> Scratchpad<T> {
    pub(crate) fn access(&self) -> &T {
        &self.access
    }

    pub(crate) fn into_parts(self) -> (String, T) {
        (self.namespace, self.access)
    }
}

impl<T: RawAccess> Scratchpad<T> {
    /// Creates a scratchpad in the specified namespace.
    pub fn new(namespace: impl Into<String>, access: T) -> Self {
        Self {
            namespace: namespace.into(),
            access,
        }
    }

    fn get_scratchpad_addr(&self, addr: IndexAddress) -> IndexAddress {
        let prefixed_addr = addr.prepend_name(&self.namespace);
        IndexAddress::from_root(SCRATCHPAD_NAME).append_key(&prefixed_addr.fully_qualified_name())
    }

    fn get_scratchpad_prefix(&self, addr: IndexAddress) -> IndexAddress {
        let prefixed_addr = addr.prepend_name(&self.namespace);
        IndexAddress::from_root(SCRATCHPAD_NAME).append_key(&prefixed_addr.qualified_prefix())
    }
}

impl<T: RawAccessMut> Scratchpad<T> {
    /// Removes all indexes and their data from the scratchpad.
    ///
    /// # Panics
    ///
    /// This operation will panic if any of the removed indexes are borrowed.
    fn clear(&self) {
        let addr = self.get_scratchpad_addr(IndexAddress::default());
        let addr = addr.append_key(&b'.');
        let removed = IndexesPool::new(self.access.clone()).remove_indexes(&addr);
        for resolved_addr in removed {
            View::new(self.access.clone(), resolved_addr).clear();
        }
    }
}

impl<T: RawAccess> Access for Scratchpad<T> {
    type Base = T;

    fn get_index_metadata(self, addr: IndexAddress) -> Result<Option<IndexMetadata>, AccessError> {
        let addr = self.get_scratchpad_addr(addr);
        Ok(ViewWithMetadata::get_metadata_unchecked(self.access, &addr))
    }

    fn get_or_create_view(
        self,
        addr: IndexAddress,
        index_type: IndexType,
    ) -> Result<ViewWithMetadata<Self::Base>, AccessError> {
        // Since we transform the address into `id_in_group`, we need to ensure that addresses
        // cannot alias each other. We do this by running the sanity check on the original address.
        if let Err(kind) = check_index_valid_full_name(addr.name()) {
            return Err(AccessError { addr, kind });
        }
        let addr = self.get_scratchpad_addr(addr);
        ViewWithMetadata::get_or_create_unchecked(self.access, &addr, index_type)
    }

    fn group_keys<K>(self, base_addr: IndexAddress) -> GroupKeys<Self::Base, K>
    where
        K: BinaryKey + ?Sized,
        Self::Base: AsReadonly<Readonly = Self::Base>,
    {
        let base_addr = self.get_scratchpad_prefix(base_addr);
        self.access.group_keys(base_addr)
    }
}

/// Migration helper.
///
/// # Examples
///
/// See the [module docs](index.html) for a basic example of usage.
///
/// ## Aborting migration
///
/// `MigrationHelper` offers [`AbortHandle`] to abort migration logic. Once aborted, `MigrationHelper`
/// does not allow to merge changes to the database; the relevant methods will return
/// [`MigrationError::Aborted`]. This is important, e.g., to prevent unnecessary writes
/// to the database.
///
/// [`AbortHandle`]: struct.AbortHandle.html
/// [`MigrationError::Aborted`]: enum.MigrationError.html#variant.Aborted
///
/// ```
/// # use assert_matches::assert_matches;
/// # use exonum_merkledb::{access::CopyAccessExt, TemporaryDB};
/// # use exonum_merkledb::migration::{MigrationHelper, MigrationError};
/// # use std::{sync::mpsc, thread, time::Duration};
/// let db = TemporaryDB::new();
/// // Since `MigrationHelper` cannot be sent between threads, we instantiate it
/// // in a newly spawned thread and move the helper handle to the main thread.
/// let (tx, rx) = mpsc::channel();
/// let helper_thread = thread::spawn(move || {
///     let (mut helper, handle) = MigrationHelper::with_handle(db, "test");
///     tx.send(handle).unwrap();
///     // Emulate some work...
///     thread::sleep(Duration::from_millis(50));
///     // Attempt to merge changes to DB.
///     helper.merge()
/// });
///
/// let handle = rx.recv().unwrap();
/// // Migration is automatically aborted when the handle is dropped.
/// drop(handle);
/// let res: Result<(), MigrationError> = helper_thread.join().unwrap();
/// assert_matches!(res, Err(MigrationError::Aborted));
/// ```
///
/// ## Using persistent iterators
///
/// `MigrationHelper` offers the [`iter_loop`](#method.iter_loop) method, which allows to further
/// simplify working with [persistent iterators].
///
/// Say we want to migrate `MapIndex` data to a `ProofMapIndex` while merging changes to the DB
/// from time to time. To do this, we use the following script:
///
/// ```
/// # use exonum_merkledb::{access::AccessExt, TemporaryDB};
/// # use exonum_merkledb::migration::{MigrationHelper, MigrationError};
/// # fn main() -> Result<(), MigrationError> {
/// /// Number of accounts processed per DB merge.
/// const CHUNK_SIZE: usize = 100;
///
/// let db = TemporaryDB::new();
/// let mut helper = MigrationHelper::new(db, "test");
/// helper.iter_loop(|helper, iters| {
///     // The data before migration is stored in this map
///     let old_map = helper.old_data().get_map::<_, str, u64>("wallets");
///     // ...and the new data is in this merkelized map.
///     let mut new_map = helper.new_data().get_proof_map::<_, str, u64>("wallets");
///
///     // Create an iterator over the old data.
///     let iter = iters.create("wallets", &old_map);
///     // Take a fixed amount of records from the iterator and migrate them.
///     // Since `iter` is persistent, it will not return the same record twice,
///     // even if this script is restarted.
///     for (name, balance) in iter.take(CHUNK_SIZE) {
///         new_map.put(&name, balance);
///     }
/// })?;
/// // Here, the iterator has run out of items. The script can now perform
/// // other actions if necessary.
/// # Ok(())
/// # }
/// ```
///
/// [persistent iterators]: struct.PersistentIter.html
pub struct MigrationHelper {
    db: Arc<dyn Database>,
    abort_handle: Box<dyn AbortMigration>,
    // Only equals `None` during merges.
    fork: Option<Fork>,
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
        Self {
            fork: Some(db.fork()),
            db,
            abort_handle: Box::new(()),
            namespace: namespace.to_owned(),
        }
    }

    /// Creates a new helper together with the abort handle. Unlike the `MigrationHelper`,
    /// the handle may be sent between threads. The handle allows to determine whether the migration
    /// helper was completed, and allows to abort the migration by preventing further writes
    /// to the database.
    pub fn with_handle(db: impl Into<Arc<dyn Database>>, namespace: &str) -> (Self, AbortHandle) {
        let mut this = Self::new(db, namespace);
        let abort_handle = AbortHandle {
            inner: Arc::new(AtomicBool::default()),
        };
        this.set_abort_handle(abort_handle.clone_inner());
        (this, abort_handle)
    }

    /// Sets the abort handle for the helper.
    ///
    /// # Stability
    ///
    /// This method is considered experimental. Its signature may be changed or it may be removed
    /// in the future.
    #[doc(hidden)]
    pub fn set_abort_handle(&mut self, abort_handle: impl AbortMigration + 'static) {
        self.abort_handle = Box::new(abort_handle);
    }

    fn fork_ref(&self) -> &Fork {
        // `unwrap` is safe due to the way we define `fork`
        self.fork.as_ref().unwrap()
    }

    /// Checks if the migration has been aborted.
    fn is_aborted(&self) -> bool {
        self.abort_handle.is_aborted()
    }

    /// Returns full access to the new version of migrated data.
    pub fn new_data(&self) -> Migration<&Fork> {
        Migration::new(&self.namespace, self.fork_ref())
    }

    /// Returns the scratchpad for temporary data to use during migration.
    pub fn scratchpad(&self) -> Scratchpad<&Fork> {
        Scratchpad::new(&self.namespace, self.fork_ref())
    }

    /// Returns readonly access to the old version of migrated data.
    pub fn old_data(&self) -> Prefixed<ReadonlyFork<'_>> {
        Prefixed::new(&self.namespace, self.fork_ref().readonly())
    }

    /// Merges the changes to the migrated data and the scratchpad to the database. Returns an error
    /// if the merge has failed.
    ///
    /// `merge` does not flush the migration; the migrated data remains in a separate namespace.
    /// Use [`flush_migration`] to flush the migrated data.
    ///
    /// [`flush_migration`]: fn.flush_migration.html
    pub fn merge(&mut self) -> Result<(), MigrationError> {
        let fork = self.fork.take().unwrap();
        let patch = fork.into_patch();
        if self.is_aborted() {
            Err(MigrationError::Aborted)
        } else {
            self.db.merge(patch).map_err(MigrationError::Merge)?;
            self.fork = Some(self.db.fork());
            Ok(())
        }
    }

    /// Executes the provided closure in a loop until all persistent iterators instantiated
    /// within the closure have ended. After each iteration, the changes in migrated data are
    /// merged to the database; an error is returned if this merge fails.
    ///
    /// If no iterators are instantiated within the closure, a single iteration will be performed.
    pub fn iter_loop(
        &mut self,
        mut step: impl FnMut(&Self, &mut PersistentIters<Scratchpad<&Fork>>),
    ) -> Result<(), MigrationError> {
        let mut should_break = false;
        while !should_break {
            let mut iterators = PersistentIters::new(self.scratchpad());
            step(self, &mut iterators);
            should_break = iterators.all_ended();
            self.merge()?;
        }
        Ok(())
    }

    /// Merges the changes to the migrated data and the migration scratchpad to the database.
    /// Returns hash representing migrated data state, or an error if the merge has failed.
    ///
    /// `finish` does not flush the migration; the migrated data remains in a separate namespace.
    /// Use [`flush_migration`] to flush the migrated data.
    ///
    /// [`flush_migration`]: fn.flush_migration.html
    pub fn finish(mut self) -> Result<Hash, MigrationError> {
        let patch = self.fork.take().unwrap().into_patch();
        let hash = Migration::new(&self.namespace, &patch).state_hash();
        if self.is_aborted() {
            Err(MigrationError::Aborted)
        } else {
            self.db.merge(patch).map_err(MigrationError::Merge)?;
            Ok(hash)
        }
    }
}

/// Errors emitted by `MigrationHelper` methods.
#[derive(Debug, Fail)]
pub enum MigrationError {
    /// Failed to merge migration changes to database.
    #[fail(display = "Failed to merge migration changes to database: {}", _0)]
    Merge(#[fail(cause)] crate::Error),

    /// Migration has been aborted.
    #[fail(display = "Migration was aborted")]
    Aborted,
}

/// Denotes a communication channel between `MigrationHelper` and the outside world allowing
/// the helper to understand if the migration is aborted.
#[doc(hidden)]
pub trait AbortMigration: Send {
    /// Has the migration been aborted? `MigrationHelper` calls this method every time before it
    /// merges changes to the database. If the method returns `true`, the merge is cancelled
    /// and `MigrationHelper` returns `MigrationError::Aborted`.
    fn is_aborted(&self) -> bool;
}

impl AbortMigration for () {
    fn is_aborted(&self) -> bool {
        false
    }
}

impl AbortMigration for AbortHandle {
    fn is_aborted(&self) -> bool {
        self.inner.load(Ordering::SeqCst)
    }
}

/// Handle allowing to signal to `MigrationHelper` that the migration has been aborted.
/// Signalling is performed on handle drop, unless it is performed with [`forget`](#method.forget)
/// method.
#[derive(Debug)]
pub struct AbortHandle {
    inner: Arc<AtomicBool>,
}

impl Drop for AbortHandle {
    fn drop(&mut self) {
        self.inner.store(true, Ordering::SeqCst);
    }
}

impl AbortHandle {
    fn clone_inner(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }

    /// Returns `true` if the `MigrationHelper` associated with this handle has been dropped.
    pub fn is_finished(&self) -> bool {
        Arc::strong_count(&self.inner) <= 1
    }

    /// Drops the handle without aborting the migration.
    pub fn forget(mut self) {
        self.inner = Arc::new(AtomicBool::default());
    }
}

/// Flushes the migration to the fork. Once the `fork` is merged, the migration is complete.
///
/// The following operations will be performed:
///
/// - Migrated indexes will replace their old versions
/// - Migrated indexes will be aggregated in the default namespace
/// - Indexes marked with tombstones will be removed
/// - Scratchpad associated with the migration will be cleared
///
/// # Safety
///
/// Flushing a migration must be performed on a `fork` which contains the final migration
/// changes. Not doing so **may break the state aggregation in the database.** A scenario when
/// this requirement would be violated is as follows:
///
/// 1. Start a database migration in a separate thread, constructing a `MigrationHelper` around
///   `Arc<dyn Database>`.
/// 2. Create a fork.
/// 3. Ensure that the migration is complete via some synchronization primitive.
/// 4. Call `flush_migration` on the fork from step 2.
///
/// In this scenario, a fork may not have the latest migration data because it was created before
/// the migration is complete. The correct workflow would be to swap steps 2 and 3, i.e.,
/// first ensure that the migration is complete and *then* create a fork in which it will be flushed.
pub fn flush_migration(fork: &mut Fork, namespace: &str) {
    fork.flush_migration(namespace);
    Scratchpad::new(namespace, &*fork).clear();
}

/// Rolls back the migration.
///
/// The following operations will be performed:
///
/// - Migrated indexes will be erased (both data and metadata)
/// - Scratchpad associated with the migration will be cleared
pub fn rollback_migration(fork: &mut Fork, namespace: &str) {
    fork.rollback_migration(namespace);
    Scratchpad::new(namespace, &*fork).clear();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        access::{AccessExt, CopyAccessExt, RawAccess},
        HashTag, ObjectHash, SystemSchema, TemporaryDB,
    };

    use assert_matches::assert_matches;
    use std::{collections::HashMap, iter::FromIterator, sync::mpsc, thread, time::Duration};

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
        migration.get_key_set("new").insert(&0_u8);
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
        migration.get_key_set("new").insert(&0_u8);
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

    #[test]
    fn concurrent_borrow_of_original_and_migrated_index() {
        let db = TemporaryDB::new();
        let helper = MigrationHelper::new(db, "test");
        let old_entry = helper.old_data().get_proof_entry::<_, u32>("entry");
        assert_eq!(old_entry.get(), None);
        let mut new_entry = helper.new_data().get_proof_entry::<_, u32>("entry");
        new_entry.set(1);
        assert_eq!(old_entry.get(), None);
    }

    #[test]
    fn scratchpad_basics() {
        let db = TemporaryDB::new();
        let fork = db.fork();
        let scratchpad = Scratchpad::new("test", &fork);
        scratchpad.get_entry("entry").set(1_u8);
        assert_eq!(scratchpad.get_entry::<_, u8>("entry").get(), Some(1));

        // Check entry address.
        {
            let addr: IndexAddress = (SCRATCHPAD_NAME, "test.entry").into();
            let view =
                ViewWithMetadata::get_or_create_unchecked(&fork, &addr, IndexType::Entry).unwrap();
            let (view, _) = view.into_parts::<()>();
            assert_eq!(view.get::<_, u8>(&()), Some(1));
        }

        scratchpad.get_list("list").extend(vec![2_u32, 3]);

        // Check that info persists to `Patch`es and `Snapshot`s.
        let patch = fork.into_patch();
        let scratchpad = Scratchpad::new("test", &patch);
        let list = scratchpad.get_list::<_, u32>("list");
        assert_eq!(list.len(), 2);
        assert_eq!(list.iter().collect::<Vec<_>>(), vec![2, 3]);
        db.merge(patch).unwrap();
        let snapshot = db.snapshot();
        let scratchpad = Scratchpad::new("test", &snapshot);
        let list = scratchpad.get_list::<_, u32>("list");
        assert_eq!(list.len(), 2);
        assert_eq!(list.iter().collect::<Vec<_>>(), vec![2, 3]);
    }

    #[test]
    fn scratchpad_address_resolution() {
        let db = TemporaryDB::new();
        let fork = db.fork();
        let scratchpad = Scratchpad::new("test", &fork);
        scratchpad.get_entry(("entry", &5_u32)).set(1_u8);

        let addr: IndexAddress = (SCRATCHPAD_NAME, &b"test.entry\0\0\0\0\x05"[..]).into();
        let view =
            ViewWithMetadata::get_or_create_unchecked(&fork, &addr, IndexType::Entry).unwrap();
        let (view, _) = view.into_parts::<()>();
        assert_eq!(view.get::<_, u8>(&()), Some(1));
    }

    #[test]
    #[should_panic(expected = "Invalid characters used in name")]
    fn scratchpad_invalid_address() {
        let db = TemporaryDB::new();
        let fork = db.fork();
        let scratchpad = Scratchpad::new("test", &fork);
        scratchpad.get_entry("entry\0key").set(1_u8);
    }

    #[test]
    fn clearing_scratchpad() {
        let db = TemporaryDB::new();
        let fork = db.fork();
        let scratchpad = Scratchpad::new("test", &fork);
        scratchpad.get_entry("entry").set(1_u8);
        scratchpad
            .get_proof_entry(("other_entry", &1_u32))
            .set("!!".to_owned());
        scratchpad.get_list("list").extend(vec![1, 2, 3]);
        scratchpad.clear();

        let scratchpad = Scratchpad::new("test", &fork);
        assert_eq!(scratchpad.index_type("entry"), None);
        assert_eq!(scratchpad.index_type(("other_entry", &1_u32)), None);
        assert_eq!(scratchpad.index_type("list"), None);

        let mut list = scratchpad.get_proof_list::<_, u32>("list");
        assert!(list.is_empty());
        assert_eq!(list.object_hash(), HashTag::empty_list_hash());
        list.extend(vec![1, 2, 3]);
        assert_eq!(list.len(), 3);
        assert_eq!(list.iter().collect::<Vec<_>>(), vec![1, 2, 3]);
    }

    #[test]
    fn clearing_scratchpad_does_not_influence_other_scratchpads() {
        let db = TemporaryDB::new();
        let fork = db.fork();
        let scratchpad = Scratchpad::new("test", &fork);
        scratchpad.get_entry("entry").set(1_u8);
        scratchpad.get_list("list").extend(vec![1, 2, 3]);
        let other_scratchpad = Scratchpad::new("test_", &fork);
        other_scratchpad.get_proof_entry("entry").set(2_u8);

        scratchpad.clear();
        let other_scratchpad = Scratchpad::new("test_", &fork);
        assert_eq!(
            other_scratchpad.get_proof_entry::<_, u8>("entry").get(),
            Some(2)
        );
    }

    #[test]
    fn scratchpad_is_cleared_after_migration() {
        let db = Arc::new(TemporaryDB::new());
        let mut helper = MigrationHelper::new(Arc::clone(&db) as Arc<dyn Database>, "test");
        helper.scratchpad().get_entry("entry").set(1_u8);
        helper.merge().unwrap();
        assert_eq!(
            helper.scratchpad().get_entry::<_, u8>("entry").get(),
            Some(1)
        );

        let mut fork = db.fork();
        flush_migration(&mut fork, "test");
        assert_eq!(Scratchpad::new("test", &fork).index_type("entry"), None);

        let helper = MigrationHelper::new(Arc::clone(&db) as Arc<dyn Database>, "test");
        helper.scratchpad().get_entry("entry").set(1_u8);
        rollback_migration(&mut fork, "test");
        assert_eq!(Scratchpad::new("test", &fork).index_type("entry"), None);
    }

    #[test]
    fn loop_iter_simple() -> Result<(), MigrationError> {
        const CHUNK_SIZE: usize = 2;
        const DATA: &[(&str, u64)] = &[
            ("Alice", 100),
            ("Bob", 75),
            ("Carol", 11),
            ("Dave", 99),
            ("Eve", 42),
        ];

        let db = TemporaryDB::new();
        // Create initial data for migration.
        let fork = db.fork();
        {
            let mut map = fork.get_map("test.balances");
            for &(name, balance) in DATA {
                map.put(name, balance);
            }
        }
        db.merge(fork.into_patch()).unwrap();

        let mut helper = MigrationHelper::new(db, "test");
        helper.iter_loop(|helper, iters| {
            let balances = helper.old_data().get_map::<_, str, u64>("balances");
            let mut new_balances = helper.new_data().get_proof_map::<_, str, u64>("balances");
            for (name, balance) in iters.create("balances", &balances).take(CHUNK_SIZE) {
                new_balances.put(&name, balance + 10);
            }
        })?;

        // Check the data after migration.
        let old_balances: HashMap<_, _> = HashMap::from_iter(DATA.iter().copied());
        let new_balances = helper.new_data().get_proof_map::<_, str, u64>("balances");
        for (name, balance) in &new_balances {
            assert_eq!(balance, old_balances[&name.as_str()] + 10);
        }

        Ok(())
    }

    struct MigrationRig {
        thread_handle: thread::JoinHandle<Result<Hash, MigrationError>>,
        abort_handle: AbortHandle,
    }

    impl MigrationRig {
        fn new(db: &Arc<TemporaryDB>) -> Self {
            let db = Arc::clone(db) as Arc<dyn Database>;
            let (tx, rx) = mpsc::channel();
            let thread_handle = thread::spawn(move || {
                let (helper, handle) = MigrationHelper::with_handle(db, "test");
                tx.send(handle).unwrap();
                thread::sleep(Duration::from_millis(50));
                helper.new_data().get_entry("entry").set(1_u32);
                helper.finish()
            });

            Self {
                thread_handle,
                abort_handle: rx.recv().unwrap(),
            }
        }
    }

    #[test]
    fn aborting_migration() {
        let db = Arc::new(TemporaryDB::new());
        let rig = MigrationRig::new(&db);
        drop(rig.abort_handle);

        let res = rig.thread_handle.join().unwrap();
        assert_matches!(res.unwrap_err(), MigrationError::Aborted);
        let snapshot = db.snapshot();
        let migration = Migration::new("test", &snapshot);
        assert!(!migration.get_entry::<_, u32>("entry").exists());
    }

    #[test]
    fn forgetting_abort_handle() {
        let db = Arc::new(TemporaryDB::new());
        let rig = MigrationRig::new(&db);
        rig.abort_handle.forget();

        let res = rig.thread_handle.join().unwrap();
        res.unwrap();
        let snapshot = db.snapshot();
        let migration = Migration::new("test", &snapshot);
        assert_eq!(migration.get_entry::<_, u32>("entry").get(), Some(1));
    }

    #[test]
    fn abort_handle_is_finished() {
        let db = Arc::new(TemporaryDB::new());
        let rig = MigrationRig::new(&db);
        assert!(!rig.abort_handle.is_finished());
        rig.thread_handle.join().unwrap().unwrap();
        assert!(rig.abort_handle.is_finished());
    }
}
