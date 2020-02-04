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

use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap, HashSet},
    fmt,
    iter::{Iterator as StdIterator, Peekable},
    marker::PhantomData,
    mem,
    ops::{Bound, Deref, DerefMut},
    rc::Rc,
    result::Result as StdResult,
};

use crate::{
    validation::assert_valid_name_component,
    views::{
        get_object_hash, AsReadonly, ChangesIter, IndexesPool, RawAccess, ResolvedAddress, View,
    },
    Error, Result, SystemSchema,
};

/// Changes related to a specific `View`.
#[derive(Debug, Default, Clone)]
pub struct ViewChanges {
    /// Changes within the view.
    pub(super) data: BTreeMap<Vec<u8>, Change>,
    /// Was the view cleared as a part of changes?
    is_cleared: bool,
    /// Is the view aggregated into `state_hash` of the database?
    /// Storing this information directly in the changes allows to avoid relatively expensive
    /// metadata lookups during state aggregator update in `Fork::into_patch()`.
    namespace: Option<String>,
}

impl ViewChanges {
    fn new() -> Self {
        Self::default()
    }

    pub fn is_cleared(&self) -> bool {
        self.is_cleared
    }

    #[cfg(test)]
    pub fn is_aggregated(&self) -> bool {
        self.namespace.as_ref().map_or(false, String::is_empty)
    }

    pub fn clear(&mut self) {
        self.data.clear();
        self.is_cleared = true;
    }

    pub fn set_aggregation(&mut self, namespace: Option<String>) {
        self.namespace = namespace;
    }

    pub(crate) fn into_data(self) -> BTreeMap<Vec<u8>, Change> {
        self.data
    }

    /// Returns a value for the specified key, or an `Err(_)` if the value should be determined
    /// by the underlying snapshot.
    pub fn get(&self, key: &[u8]) -> StdResult<Option<Vec<u8>>, ()> {
        if let Some(change) = self.data.get(key) {
            return Ok(match *change {
                Change::Put(ref v) => Some(v.clone()),
                Change::Delete => None,
            });
        }
        if self.is_cleared() {
            return Ok(None);
        }
        Err(())
    }

    /// Returns whether the view contains the specified `key`. An `Err(_)` is returned if this
    /// is determined by the underlying snapshot.
    pub fn contains(&self, key: &[u8]) -> StdResult<bool, ()> {
        if let Some(change) = self.data.get(key) {
            return Ok(match *change {
                Change::Put(..) => true,
                Change::Delete => false,
            });
        }

        if self.is_cleared() {
            return Ok(false);
        }
        Err(())
    }
}

/// Cell holding changes for a specific view. Mutable view borrows take changes out
/// of the `Option` and unwraps `Rc` into inner data, while immutable borrows clone inner `Rc`.
type ChangesCell = Option<Rc<ViewChanges>>;

#[derive(Debug, Default)]
struct WorkingPatch {
    changes: RefCell<HashMap<ResolvedAddress, ChangesCell>>,
}

#[derive(Debug)]
enum WorkingPatchRef<'a> {
    Borrowed(&'a WorkingPatch),
    Owned(Rc<Fork>),
}

impl WorkingPatchRef<'_> {
    fn patch(&self) -> &WorkingPatch {
        match self {
            WorkingPatchRef::Borrowed(patch) => patch,
            WorkingPatchRef::Owned(ref fork) => &fork.working_patch,
        }
    }
}

#[derive(Debug)]
pub struct ChangesRef<'a> {
    inner: Rc<ViewChanges>,
    _lifetime: PhantomData<&'a ()>,
}

impl Drop for ChangesRef<'_> {
    fn drop(&mut self) {
        // Do nothing. The implementation is required to make `View`s based on `ChangesRef`
        // drop before a mutable operation is performed on a fork (e.g., it's converted
        // into a patch).
    }
}

impl Deref for ChangesRef<'_> {
    type Target = ViewChanges;

    fn deref(&self) -> &ViewChanges {
        &*self.inner
    }
}

/// `RefMut`, but dumber.
#[derive(Debug)]
pub struct ChangesMut<'a> {
    parent: WorkingPatchRef<'a>,
    key: ResolvedAddress,
    changes: Option<Rc<ViewChanges>>,
}

impl Deref for ChangesMut<'_> {
    type Target = ViewChanges;

    fn deref(&self) -> &ViewChanges {
        // `.unwrap()` is safe: `changes` can be equal to `None` only when
        // the instance is being dropped.
        self.changes.as_ref().unwrap()
    }
}

impl DerefMut for ChangesMut<'_> {
    fn deref_mut(&mut self) -> &mut ViewChanges {
        // `.unwrap()`s are safe:
        //
        // - `changes` can be equal to `None` only when the instance is being dropped.
        // - We know that `Rc` with the changes is unique.
        Rc::get_mut(self.changes.as_mut().unwrap()).unwrap()
    }
}

impl Drop for ChangesMut<'_> {
    fn drop(&mut self) {
        let mut change_map = self.parent.patch().changes.borrow_mut();
        let changes = change_map.get_mut(&self.key).unwrap_or_else(|| {
            panic!("insertion point for changes disappeared at {:?}", self.key);
        });

        debug_assert!(changes.is_none(), "edit conflict at {:?}", self.key);
        *changes = self.changes.take();
    }
}

impl WorkingPatch {
    /// Creates a new empty patch.
    fn new() -> Self {
        Self {
            changes: RefCell::new(HashMap::new()),
        }
    }

    /// Takes a cell with changes for a specific `View` out of the patch.
    /// The returned cell is guaranteed to contain an `Rc` with an exclusive ownership.
    fn take_view_changes(&self, address: &ResolvedAddress) -> ChangesCell {
        let view_changes = {
            let mut changes = self.changes.borrow_mut();
            let view_changes = changes.get_mut(address).map(Option::take);
            view_changes.unwrap_or_else(|| {
                changes
                    .entry(address.clone())
                    .or_insert_with(|| Some(Rc::new(ViewChanges::new())))
                    .take()
            })
        };

        if let Some(ref view_changes) = view_changes {
            assert!(
                Rc::strong_count(view_changes) == 1,
                "Attempting to borrow {:?} mutably while it's borrowed immutably",
                address
            );
        } else {
            panic!("Multiple mutable borrows of an index at {:?}", address);
        }
        view_changes
    }

    /// Clones changes for a specific `View` from the patch. Panics if the changes
    /// are mutably borrowed.
    fn clone_view_changes(&self, address: &ResolvedAddress) -> Rc<ViewChanges> {
        let mut changes = self.changes.borrow_mut();
        // Get changes for the specified address.
        let changes: &ChangesCell = changes
            .entry(address.clone())
            .or_insert_with(|| Some(Rc::new(ViewChanges::new())));

        changes
            .as_ref()
            .unwrap_or_else(|| {
                // If the `changes` are `None`, this means they have been taken by a previous call
                // to `take_view_changes` and not yet returned.
                panic!(
                    "Attempting to borrow {:?} immutably while it's borrowed mutably",
                    address
                );
            })
            .clone()
    }

    // TODO: verify that this method updates `Change`s already in the `Patch` [ECR-2834]
    fn merge_into(self, patch: &mut Patch) {
        for (address, changes) in self.changes.into_inner() {
            // Check that changes are not borrowed mutably (in this case, the corresponding
            // `ChangesCell` is `None`).
            //
            // Both this and the following `panic`s cannot feasibly be triggered,
            // since the only place where this method is called (`Fork::flush()`) borrows
            // `Fork` mutably; this forces both mutable and immutable index borrows to be dropped,
            // since they borrow `Fork` immutably.
            let changes = changes.unwrap_or_else(|| {
                panic!(
                    "changes are still mutably borrowed at address {:?}",
                    address
                );
            });
            // Check that changes are not borrowed immutably (in this case, there is another
            // `Rc<_>` pointer to changes somewhere).
            let mut changes = Rc::try_unwrap(changes).unwrap_or_else(|_| {
                panic!(
                    "changes are still immutably borrowed at address {:?}",
                    address
                );
            });

            if let Some(namespace) = mem::replace(&mut changes.namespace, None) {
                patch
                    .changed_aggregated_addrs
                    .insert(address.clone(), namespace);
            }

            // The patch may already contain changes related to the `address`. If it does,
            // we extend these changes with the new changes (relying on the fact that
            // newer changes override older ones), unless the view was cleared (in which case,
            // the old changes do not matter and should be forgotten).
            let patch_changes = patch
                .changes
                .entry(address)
                .or_insert_with(ViewChanges::new);
            if changes.is_cleared() {
                *patch_changes = changes;
            } else {
                patch_changes.data.extend(changes.data);
            }
        }
    }
}

/// A generalized iterator over the storage views.
pub type Iter<'a> = Box<dyn Iterator + 'a>;

/// An enum that represents a type of change made to some key in the storage.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(test, derive(Eq, Hash))] // needed for patch equality comparison
pub(crate) enum Change {
    /// Put the specified value into the storage for the corresponding key.
    Put(Vec<u8>),
    /// Delete a value from the storage for the corresponding key.
    Delete,
}

/// A combination of a database snapshot and changes on top of it.
///
/// A `Fork` provides both immutable and mutable operations over the database by implementing
/// the [`RawAccessMut`] trait. Like [`Snapshot`], `Fork` provides read isolation.
/// When mutable operations are applied to a fork, the subsequent reads act as if the changes
/// are applied to the database; in reality, these changes are accumulated in memory.
///
/// To apply the changes to the database, you need to convert a `Fork` into a [`Patch`] using
/// [`into_patch`] and then atomically [`merge`] it into the database. If two
/// conflicting forks are merged into a database, this can lead to an inconsistent state. If you
/// need to consistently apply several sets of changes to the same data, the next fork should be
/// created after the previous fork has been merged.
///
/// `Fork` also supports checkpoints ([`flush`] and [`rollback`] methods), which allows
/// rolling back the latest changes. A checkpoint is created automatically after calling
/// the `flush` method.
///
/// ```
/// # use exonum_merkledb::{access::CopyAccessExt, Database, TemporaryDB};
/// let db = TemporaryDB::new();
/// let mut fork = db.fork();
/// fork.get_list("list").extend(vec![1_u32, 2]);
/// fork.flush();
/// fork.get_list("list").push(3_u32);
/// fork.rollback();
/// // The changes after the latest `flush()` are now forgotten.
/// let list = fork.get_list::<_, u32>("list");
/// assert_eq!(list.len(), 2);
/// # assert_eq!(list.iter().collect::<Vec<_>>(), vec![1, 2]);
/// ```
///
/// In order to convert a fork into `&dyn Snapshot` presentation, convert it into a `Patch`
/// and use a reference to it (`Patch` implements `Snapshot`). Using `<Fork as RawAccess>::snapshot`
/// for this purpose is logically incorrect and may lead to hard-to-debug errors.
///
/// # Borrow checking
///
/// It is possible to create only one instance of index with the specified `IndexAddress` based on a
/// single fork. If an additional instance is requested, the code will panic in runtime.
/// Hence, obtaining indexes from a `Fork` functions similarly to [`RefCell::borrow_mut()`].
///
/// For example the code below will panic at runtime.
///
/// ```rust,should_panic
/// # use exonum_merkledb::{access::CopyAccessExt, TemporaryDB, ListIndex, Database};
/// let db = TemporaryDB::new();
/// let fork = db.fork();
/// let index = fork.get_list::<_, u8>("index");
/// // This code will panic at runtime.
/// let index2 = fork.get_list::<_, u8>("index");
/// ```
///
/// To enable immutable / shared references to indexes, you may use [`readonly`] method:
///
/// ```
/// # use exonum_merkledb::{access::CopyAccessExt, TemporaryDB, ListIndex, Database};
/// let db = TemporaryDB::new();
/// let fork = db.fork();
/// fork.get_list::<_, u8>("index").extend(vec![1, 2, 3]);
///
/// let readonly = fork.readonly();
/// let index = readonly.get_list::<_, u8>("index");
/// // Works fine.
/// let index2 = readonly.get_list::<_, u8>("index");
/// ```
///
/// It is impossible to mutate index contents having a readonly access to the fork; this is
/// checked by the Rust type system.
///
/// Shared references work like `RefCell::borrow()`; it is a runtime error to try to obtain
/// a shared reference to an index if there is an exclusive reference to the same index,
/// and vice versa.
///
/// [`RawAccessMut`]: access/trait.RawAccessMut.html
/// [`Snapshot`]: trait.Snapshot.html
/// [`Patch`]: struct.Patch.html
/// [`into_patch`]: #method.into_patch
/// [`merge`]: trait.Database.html#tymethod.merge
/// [`commit`]: #method.commit
/// [`flush`]: #method.flush
/// [`rollback`]: #method.rollback
/// [`readonly`]: #method.readonly
/// [`RefCell::borrow_mut()`]: https://doc.rust-lang.org/std/cell/struct.RefCell.html#method.borrow_mut
#[derive(Debug)]
pub struct Fork {
    patch: Patch,
    working_patch: WorkingPatch,
}

/// A set of changes that can be atomically applied to a `Database`.
///
/// This set can contain changes from multiple indexes. Changes can be read from the `Patch`
/// using its `RawAccess` implementation.
///
/// # Examples
///
/// ```
/// # use exonum_merkledb::{
/// #     access::CopyAccessExt, Database, ObjectHash, Patch, SystemSchema, TemporaryDB,
/// # };
/// let db = TemporaryDB::new();
/// let fork = db.fork();
/// fork.get_proof_list("list").extend(vec![1_i32, 2, 3]);
/// let patch: Patch = fork.into_patch();
/// // The patch contains changes recorded in the fork.
/// let list = patch.get_proof_list::<_, i32>("list");
/// assert_eq!(list.len(), 3);
/// // Unlike `Fork`, `Patch`es have consistent aggregated state.
/// let aggregator = SystemSchema::new(&patch).state_aggregator();
/// assert_eq!(aggregator.get("list").unwrap(), list.object_hash());
/// ```
#[derive(Debug)]
pub struct Patch {
    snapshot: Box<dyn Snapshot>,
    changes: HashMap<ResolvedAddress, ViewChanges>,
    /// Addresses of aggregated indexes that were changed within this patch. This information
    /// is used to update the state aggregator in `Fork::into_patch()`.
    changed_aggregated_addrs: HashMap<ResolvedAddress, String>,
    /// Names of removed aggregated indexes.
    removed_aggregated_addrs: HashSet<String>,
}

pub(super) struct ForkIter<'a, T: StdIterator> {
    snapshot: Iter<'a>,
    changes: Option<Peekable<T>>,
}

#[derive(Debug, PartialEq, Eq)]
enum NextIterValue {
    Stored,
    Replaced,
    Inserted,
    Deleted,
    MissDeleted,
    Finished,
}

/// Low-level storage backend implementing a collection of named key-value stores
/// (aka column families).
///
/// A `Database` instance is shared across different threads, so it must be `Sync` and `Send`.
///
/// There is no way to directly interact with data in the database; use [`snapshot`], [`fork`]
/// and [`merge`] methods for indirect interaction. See [the crate-level documentation](index.html)
/// for more details.
///
/// Note that `Database` effectively has [interior mutability][interior-mut];
/// `merge` and `merge_sync` methods take a shared reference to the database (`&self`)
/// rather than an exclusive one (`&mut self`). This means that the following code compiles:
///
/// ```
/// use exonum_merkledb::{access::CopyAccessExt, Database, TemporaryDB};
///
/// // not declared as `mut db`!
/// let db: Box<dyn Database> = Box::new(TemporaryDB::new());
/// let fork = db.fork();
/// {
///     let mut list = fork.get_proof_list("list");
///     list.push(42_u64);
/// }
/// db.merge(fork.into_patch()).unwrap();
/// ```
///
/// [`snapshot`]: #tymethod.snapshot
/// [`fork`]: #method.fork
/// [`merge`]: #tymethod.merge
/// [interior-mut]: https://doc.rust-lang.org/book/ch15-05-interior-mutability.html
pub trait Database: Send + Sync + 'static {
    /// Creates a new snapshot of the database from its current state.
    fn snapshot(&self) -> Box<dyn Snapshot>;

    /// Creates a new fork of the database from its current state.
    fn fork(&self) -> Fork {
        Fork {
            patch: Patch {
                snapshot: self.snapshot(),
                changes: HashMap::new(),
                changed_aggregated_addrs: HashMap::new(),
                removed_aggregated_addrs: HashSet::new(),
            },
            working_patch: WorkingPatch::new(),
        }
    }

    /// Atomically applies a sequence of patch changes to the database.
    ///
    /// Note that this method may be called concurrently from different threads, the
    /// onus to guarantee atomicity is on the implementor of the trait.
    ///
    /// # Errors
    ///
    /// If this method encounters any form of I/O or other error during merging, an error variant
    /// will be returned. In case of an error, the method guarantees no changes are applied to
    /// the database.
    fn merge(&self, patch: Patch) -> Result<()>;

    /// Atomically applies a sequence of patch changes to the database with fsync.
    ///
    /// Note that this method may be called concurrently from different threads, the
    /// onus to guarantee atomicity is on the implementor of the trait.
    ///
    /// # Errors
    ///
    /// If this method encounters any form of I/O or other error during merging, an error variant
    /// will be returned. In case of an error, the method guarantees no changes are applied to
    /// the database.
    fn merge_sync(&self, patch: Patch) -> Result<()>;
}

/// Extension trait for `Database`.
pub trait DatabaseExt: Database {
    /// Merges a patch into the database and creates a backup patch that reverses all the merged
    /// changes.
    ///
    /// # Safety
    ///
    /// It is logically unsound to merge other patches to the database between the `merge_with_backup`
    /// call and merging the backup patch. This may lead to merge artifacts and an inconsistent
    /// database state.
    ///
    /// An exception to this rule is creating backups for several merged patches
    /// and then applying backups in the reverse order:
    ///
    /// ```
    /// # use exonum_merkledb::{access::{Access, CopyAccessExt}, Database, DatabaseExt, TemporaryDB};
    /// let db = TemporaryDB::new();
    /// let fork = db.fork();
    /// fork.get_list("list").push(1_u32);
    /// let backup1 = db.merge_with_backup(fork.into_patch()).unwrap();
    /// let fork = db.fork();
    /// fork.get_list("list").push(2_u32);
    /// let backup2 = db.merge_with_backup(fork.into_patch()).unwrap();
    /// let fork = db.fork();
    /// fork.get_list("list").extend(vec![3_u32, 4]);
    /// let backup3 = db.merge_with_backup(fork.into_patch()).unwrap();
    ///
    /// fn enumerate_list<A: Access + Copy>(view: A) -> Vec<u32> {
    ///     view.get_list("list").iter().collect()
    /// }
    ///
    /// assert_eq!(enumerate_list(&db.snapshot()), vec![1, 2, 3, 4]);
    /// // Rollback the most recent merge.
    /// db.merge(backup3).unwrap();
    /// assert_eq!(enumerate_list(&db.snapshot()), vec![1, 2]);
    /// // ...Then the penultimate merge.
    /// db.merge(backup2).unwrap();
    /// assert_eq!(enumerate_list(&db.snapshot()), vec![1]);
    /// // ...Then the oldest one.
    /// db.merge(backup1).unwrap();
    /// assert!(enumerate_list(&db.snapshot()).is_empty());
    /// ```
    ///
    /// # Performance notes
    ///
    /// This method is linear w.r.t. patch size (i.e., the total number of changes in it) plus,
    /// for each clear operation, the corresponding index size before clearing. As such,
    /// the method may be inappropriate to use with large patches.
    ///
    /// # Errors
    ///
    /// Returns an error in the same situations as `Database::merge()`.
    fn merge_with_backup(&self, patch: Patch) -> Result<Patch> {
        // FIXME: does this work with migrations? (ECR-4005)

        let snapshot = self.snapshot();
        let changed_aggregated_addrs = patch.changed_aggregated_addrs.clone();
        let mut rev_changes = HashMap::with_capacity(patch.changes.len());

        for (name, changes) in &patch.changes {
            let mut view_changes = changes.data.clone();
            for (key, change) in &mut view_changes {
                *change = if let Some(value) = snapshot.get(name, key) {
                    Change::Put(value)
                } else {
                    Change::Delete
                };
            }

            // Remember all elements that will be deleted.
            if changes.is_cleared() {
                let mut iter = snapshot.iter(name, &[]);
                while let Some((key, value)) = iter.next() {
                    view_changes.insert(key.to_vec(), Change::Put(value.to_vec()));
                }
            }

            rev_changes.insert(
                name.to_owned(),
                ViewChanges {
                    data: view_changes,
                    is_cleared: false,
                    namespace: changes.namespace.clone(),
                },
            );
        }

        self.merge(patch)?;
        Ok(Patch {
            snapshot: self.snapshot(),
            changes: rev_changes,
            changed_aggregated_addrs,
            removed_aggregated_addrs: HashSet::new(),
        })
    }
}

impl<T: Database> DatabaseExt for T {}

/// A read-only snapshot of a storage backend.
///
/// A `Snapshot` instance is an immutable representation of a certain storage state.
/// It provides read isolation, so consistency is guaranteed even if the data in
/// the database changes between reads.
pub trait Snapshot: Send + Sync + 'static {
    /// Returns a value corresponding to the specified address and key as a raw vector of bytes,
    /// or `None` if it does not exist.
    fn get(&self, name: &ResolvedAddress, key: &[u8]) -> Option<Vec<u8>>;

    /// Returns `true` if the snapshot contains a value for the specified address and key.
    ///
    /// The default implementation checks existence of the value using [`get`](#tymethod.get).
    fn contains(&self, name: &ResolvedAddress, key: &[u8]) -> bool {
        self.get(name, key).is_some()
    }

    /// Returns an iterator over the entries of the snapshot in ascending order starting from
    /// the specified key. The iterator element type is `(&[u8], &[u8])`.
    fn iter(&self, name: &ResolvedAddress, from: &[u8]) -> Iter<'_>;
}

/// A trait that defines a streaming iterator over storage view entries. Unlike
/// the standard [`Iterator`](https://doc.rust-lang.org/std/iter/trait.Iterator.html)
/// trait, `Iterator` in Exonum is low-level and, therefore, operates with bytes.
pub trait Iterator {
    /// Advances the iterator and returns a reference to the next key and value.
    fn next(&mut self) -> Option<(&[u8], &[u8])>;

    /// Returns a reference to the current key and value without advancing the iterator.
    fn peek(&mut self) -> Option<(&[u8], &[u8])>;
}

impl Patch {
    /// Iterates over changes in this patch.
    pub(crate) fn into_changes(self) -> HashMap<ResolvedAddress, ViewChanges> {
        self.changes
    }
}

impl Snapshot for Patch {
    fn get(&self, name: &ResolvedAddress, key: &[u8]) -> Option<Vec<u8>> {
        self.changes
            .get(name)
            .map_or(Err(()), |changes| changes.get(key))
            // At this point, `Err(_)` signifies that we need to retrieve data from the snapshot.
            .unwrap_or_else(|()| self.snapshot.get(name, key))
    }

    fn contains(&self, name: &ResolvedAddress, key: &[u8]) -> bool {
        self.changes
            .get(name)
            .map_or(Err(()), |changes| changes.contains(key))
            // At this point, `Err(_)` signifies that we need to retrieve data from the snapshot.
            .unwrap_or_else(|()| self.snapshot.contains(name, key))
    }

    fn iter(&self, name: &ResolvedAddress, from: &[u8]) -> Iter<'_> {
        let maybe_changes = self.changes.get(name);
        let changes_iter = maybe_changes.map(|changes| {
            changes
                .data
                .range::<[u8], _>((Bound::Included(from), Bound::Unbounded))
        });

        let is_cleared = maybe_changes.map_or(false, ViewChanges::is_cleared);
        if is_cleared {
            // Ignore all changes from the snapshot.
            Box::new(ChangesIter::new(changes_iter.unwrap()))
        } else {
            Box::new(ForkIter::new(self.snapshot.iter(name, from), changes_iter))
        }
    }
}

impl RawAccess for &'_ Patch {
    type Changes = ();

    fn snapshot(&self) -> &dyn Snapshot {
        *self as &dyn Snapshot
    }

    fn changes(&self, _address: &ResolvedAddress) -> Self::Changes {}
}

impl AsReadonly for &'_ Patch {
    type Readonly = Self;

    fn as_readonly(&self) -> Self::Readonly {
        self
    }
}

impl Fork {
    /// Finalizes all changes that were made after previous execution of the `flush` method.
    /// If no `flush` method had been called before, finalizes all changes that were
    /// made after creation of `Fork`.
    pub fn flush(&mut self) {
        let working_patch = mem::replace(&mut self.working_patch, WorkingPatch::new());
        working_patch.merge_into(&mut self.patch);
    }

    /// Finishes a migration of indexes with the specified prefix.
    pub(crate) fn flush_migration(&mut self, prefix: &str) {
        assert_valid_name_component(prefix);

        // Mutable `self` reference ensures that no indexes are instantiated in the client code.
        self.flush(); // Flushing is necessary to keep `self.patch` up to date.

        // Update aggregation attribution of indexes.
        for namespace in self.patch.changed_aggregated_addrs.values_mut() {
            if namespace == prefix {
                namespace.clear();
            }
        }
        // Move aggregated indexes info from the `prefix` namespace into the default namespace.
        SystemSchema::new(&*self).merge_namespace(prefix);

        let removed_addrs = IndexesPool::new(&*self).flush_migration(&prefix);
        for (addr, is_removed_from_aggregation) in removed_addrs {
            self.patch.changed_aggregated_addrs.remove(&addr);
            if is_removed_from_aggregation {
                self.patch
                    .removed_aggregated_addrs
                    .insert(addr.name.clone());
            }
            self.patch.changes.entry(addr).or_default().clear();
        }
    }

    /// Rolls back all changes that were made after the latest execution
    /// of the `flush` method.
    pub fn rollback(&mut self) {
        self.working_patch = WorkingPatch::new();
    }

    /// Rolls back the migration with the specified name. This will remove all indexes
    /// within the migration.
    pub(crate) fn rollback_migration(&mut self, prefix: &str) {
        assert_valid_name_component(prefix);
        self.flush();
        SystemSchema::new(&*self).remove_namespace(prefix);
        let removed_addrs = IndexesPool::new(&*self).rollback_migration(&prefix);
        for addr in &removed_addrs {
            self.patch.changed_aggregated_addrs.remove(addr);
            self.patch.changes.remove(addr);
        }
    }

    /// Converts the fork into `Patch` consuming the fork instance.
    pub fn into_patch(mut self) -> Patch {
        self.flush();

        // Replacing `changed_aggregated_addrs` has a beneficial side-effect: if the patch
        // returned by this method is converted back to a `Fork`, we won't need to update
        // its state aggregator unless the *new* changes in the `Fork` concern aggregated indexes.
        let changed_aggregated_addrs =
            mem::replace(&mut self.patch.changed_aggregated_addrs, HashMap::new());
        let updated_entries = changed_aggregated_addrs.into_iter().map(|(addr, ns)| {
            let index_name = addr.name.clone();
            let is_in_migration = !ns.is_empty();
            let index_hash = get_object_hash(&self.patch, addr, is_in_migration);
            (ns, index_name, index_hash)
        });
        SystemSchema::new(&self).update_state_aggregators(updated_entries);

        let removed_aggregated_addrs =
            mem::replace(&mut self.patch.removed_aggregated_addrs, HashSet::new());
        SystemSchema::new(&self).remove_aggregated_indexes(removed_aggregated_addrs);

        self.flush(); // flushes changes in the state aggregator
        self.patch
    }

    /// Returns a readonly wrapper around the fork. Indexes created based on the readonly
    /// version cannot be modified; on the other hand, it is possible to have multiple
    /// copies of an index at the same time.
    pub fn readonly(&self) -> ReadonlyFork<'_> {
        ReadonlyFork(self)
    }
}

impl From<Patch> for Fork {
    /// Creates a fork based on the provided `patch` and `snapshot`.
    ///
    /// Note: using created fork to modify data already present in `patch` may lead
    /// to an inconsistent database state. Hence, this method is useful only if you
    /// are sure that the fork and `patch` interacted with different indexes.
    fn from(patch: Patch) -> Self {
        Self {
            patch,
            working_patch: WorkingPatch::new(),
        }
    }
}

impl<'a> RawAccess for &'a Fork {
    type Changes = ChangesMut<'a>;

    fn snapshot(&self) -> &dyn Snapshot {
        &self.patch
    }

    fn changes(&self, address: &ResolvedAddress) -> Self::Changes {
        let changes = self.working_patch.take_view_changes(address);
        ChangesMut {
            changes,
            key: address.clone(),
            parent: WorkingPatchRef::Borrowed(&self.working_patch),
        }
    }
}

impl RawAccess for Rc<Fork> {
    type Changes = ChangesMut<'static>;

    fn snapshot(&self) -> &dyn Snapshot {
        &self.patch
    }

    fn changes(&self, address: &ResolvedAddress) -> Self::Changes {
        let changes = self.working_patch.take_view_changes(address);
        ChangesMut {
            changes,
            key: address.clone(),
            parent: WorkingPatchRef::Owned(Self::clone(self)),
        }
    }
}

/// Readonly wrapper for a `Fork`.
///
/// This wrapper allows to read from index state from the fork
/// in a type-safe manner (it is impossible to accidentally modify data in the index), and
/// without encountering runtime errors when attempting to concurrently get the same index
/// more than once.
///
/// Since the wrapper borrows the `Fork` immutably, it is still possible to access indexes
/// in the fork directly. In this scenario, the caller should be careful that `ReadonlyFork`
/// does not access the same indexes as the original `Fork`: this will result in a runtime
/// error (sort of like attempting both an exclusive and a shared borrow from a `RefCell`
/// or `RwLock`).
///
/// # Examples
///
/// ```
/// # use exonum_merkledb::{access::CopyAccessExt, Database, ReadonlyFork, TemporaryDB};
/// let db = TemporaryDB::new();
/// let fork = db.fork();
/// fork.get_list("list").push(1_u32);
/// let readonly: ReadonlyFork<'_> = fork.readonly();
/// let list = readonly.get_list::<_, u32>("list");
/// assert_eq!(list.get(0), Some(1));
/// let same_list = readonly.get_list::<_, u32>("list");
/// // ^-- Does not result in an error!
///
/// // Original fork is still accessible.
/// let mut map = fork.get_map("map");
/// map.put(&1_u32, "foo".to_string());
/// ```
///
/// There are no write methods in indexes instantiated from `ReadonlyFork`:
///
/// ```compile_fail
/// # use exonum_merkledb::{access::CopyAccessExt, Database, ReadonlyFork, TemporaryDB};
/// let db = TemporaryDB::new();
/// let fork = db.fork();
/// let readonly: ReadonlyFork<'_> = fork.readonly();
/// let mut list = readonly.get_list("list");
/// list.push(1_u32); // Won't compile: no `push` method in `ListIndex<ReadonlyFork, u32>`!
/// ```
#[derive(Debug, Clone, Copy)]
pub struct ReadonlyFork<'a>(&'a Fork);

impl<'a> AsReadonly for ReadonlyFork<'a> {
    type Readonly = Self;

    fn as_readonly(&self) -> Self::Readonly {
        *self
    }
}

impl<'a> AsReadonly for &'a Fork {
    type Readonly = ReadonlyFork<'a>;

    fn as_readonly(&self) -> Self::Readonly {
        ReadonlyFork(*self)
    }
}

impl<'a> RawAccess for ReadonlyFork<'a> {
    type Changes = ChangesRef<'a>;

    fn snapshot(&self) -> &dyn Snapshot {
        &self.0.patch
    }

    fn changes(&self, address: &ResolvedAddress) -> Self::Changes {
        ChangesRef {
            inner: self.0.working_patch.clone_view_changes(address),
            _lifetime: PhantomData,
        }
    }
}

/// Version of `ReadonlyFork` with a static lifetime. Can be produced from an `Rc<Fork>` using
/// the `AsReadonly` trait.
///
/// Beware that producing an instance increases the reference counter of the underlying fork.
/// If you need to obtain `Fork` from `Rc<Fork>` via [`Rc::try_unwrap`], make sure that all
/// `OwnedReadonlyFork` instances are dropped by this time.
///
/// [`Rc::try_unwrap`]: https://doc.rust-lang.org/std/rc/struct.Rc.html#method.try_unwrap
///
/// # Examples
///
/// ```
/// # use exonum_merkledb::{access::AccessExt, AsReadonly, Database, OwnedReadonlyFork, TemporaryDB};
/// # use std::rc::Rc;
/// let db = TemporaryDB::new();
/// let fork = Rc::new(db.fork());
/// fork.get_proof_list("list").extend(vec![1_u32, 2, 3]);
/// let ro_fork: OwnedReadonlyFork = fork.as_readonly();
/// let list = ro_fork.get_proof_list::<_, u32>("list");
/// assert_eq!(list.len(), 3);
/// ```
#[derive(Debug, Clone)]
pub struct OwnedReadonlyFork(Rc<Fork>);

impl RawAccess for OwnedReadonlyFork {
    type Changes = ChangesRef<'static>;

    fn snapshot(&self) -> &dyn Snapshot {
        &self.0.patch
    }

    fn changes(&self, address: &ResolvedAddress) -> Self::Changes {
        ChangesRef {
            inner: self.0.working_patch.clone_view_changes(address),
            _lifetime: PhantomData,
        }
    }
}

impl AsReadonly for OwnedReadonlyFork {
    type Readonly = Self;

    fn as_readonly(&self) -> Self::Readonly {
        self.clone()
    }
}

impl AsReadonly for Rc<Fork> {
    type Readonly = OwnedReadonlyFork;

    fn as_readonly(&self) -> Self::Readonly {
        OwnedReadonlyFork(self.clone())
    }
}

impl AsRef<dyn Snapshot> for dyn Snapshot {
    fn as_ref(&self) -> &dyn Snapshot {
        self
    }
}

impl Snapshot for Box<dyn Snapshot> {
    fn get(&self, name: &ResolvedAddress, key: &[u8]) -> Option<Vec<u8>> {
        self.as_ref().get(name, key)
    }

    fn contains(&self, name: &ResolvedAddress, key: &[u8]) -> bool {
        self.as_ref().contains(name, key)
    }

    fn iter(&self, name: &ResolvedAddress, from: &[u8]) -> Iter<'_> {
        self.as_ref().iter(name, from)
    }
}

impl<'a, T> ForkIter<'a, T>
where
    T: StdIterator<Item = (&'a Vec<u8>, &'a Change)>,
{
    pub fn new(snapshot: Iter<'a>, changes: Option<T>) -> Self {
        ForkIter {
            snapshot,
            changes: changes.map(StdIterator::peekable),
        }
    }

    fn step(&mut self) -> NextIterValue {
        use std::cmp::Ordering::*;

        if let Some(ref mut changes) = self.changes {
            match changes.peek() {
                Some(&(k, change)) => match self.snapshot.peek() {
                    Some((key, ..)) => match *change {
                        Change::Put(..) => match k[..].cmp(key) {
                            Equal => NextIterValue::Replaced,
                            Less => NextIterValue::Inserted,
                            Greater => NextIterValue::Stored,
                        },
                        Change::Delete => match k[..].cmp(key) {
                            Equal => NextIterValue::Deleted,
                            Less => NextIterValue::MissDeleted,
                            Greater => NextIterValue::Stored,
                        },
                    },
                    None => match *change {
                        Change::Put(..) => NextIterValue::Inserted,
                        Change::Delete => NextIterValue::MissDeleted,
                    },
                },
                None => match self.snapshot.peek() {
                    Some(..) => NextIterValue::Stored,
                    None => NextIterValue::Finished,
                },
            }
        } else {
            match self.snapshot.peek() {
                Some(..) => NextIterValue::Stored,
                None => NextIterValue::Finished,
            }
        }
    }
}

impl<'a, T> Iterator for ForkIter<'a, T>
where
    T: StdIterator<Item = (&'a Vec<u8>, &'a Change)>,
{
    fn next(&mut self) -> Option<(&[u8], &[u8])> {
        loop {
            match self.step() {
                NextIterValue::Stored => return self.snapshot.next(),
                NextIterValue::Replaced => {
                    self.snapshot.next();
                    return self.changes.as_mut().unwrap().next().map(|(key, change)| {
                        (
                            key.as_slice(),
                            match *change {
                                Change::Put(ref value) => value.as_slice(),
                                Change::Delete => unreachable!(),
                            },
                        )
                    });
                }
                NextIterValue::Inserted => {
                    return self.changes.as_mut().unwrap().next().map(|(key, change)| {
                        (
                            key.as_slice(),
                            match *change {
                                Change::Put(ref value) => value.as_slice(),
                                Change::Delete => unreachable!(),
                            },
                        )
                    });
                }
                NextIterValue::Deleted => {
                    self.changes.as_mut().unwrap().next();
                    self.snapshot.next();
                }
                NextIterValue::MissDeleted => {
                    self.changes.as_mut().unwrap().next();
                }
                NextIterValue::Finished => return None,
            }
        }
    }

    fn peek(&mut self) -> Option<(&[u8], &[u8])> {
        loop {
            match self.step() {
                NextIterValue::Stored => return self.snapshot.peek(),
                NextIterValue::Replaced | NextIterValue::Inserted => {
                    return self.changes.as_mut().unwrap().peek().map(|&(key, change)| {
                        (
                            key.as_slice(),
                            match *change {
                                Change::Put(ref value) => value.as_slice(),
                                Change::Delete => unreachable!(),
                            },
                        )
                    });
                }
                NextIterValue::Deleted => {
                    self.changes.as_mut().unwrap().next();
                    self.snapshot.next();
                }
                NextIterValue::MissDeleted => {
                    self.changes.as_mut().unwrap().next();
                }
                NextIterValue::Finished => return None,
            }
        }
    }
}

impl fmt::Debug for dyn Database {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Database").finish()
    }
}

impl fmt::Debug for dyn Snapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Snapshot").finish()
    }
}

impl fmt::Debug for dyn Iterator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Iterator").finish()
    }
}

/// The current `MerkleDB` data layout version.
pub const DB_VERSION: u8 = 0;
/// Database metadata address.
pub const DB_METADATA: &str = "__DB_METADATA__";
/// Version attribute name.
pub const VERSION_NAME: &str = "version";

/// This function checks that the given database is compatible with the current `MerkleDB` version.
pub fn check_database(db: &mut dyn Database) -> Result<()> {
    let fork = db.fork();
    {
        let addr = ResolvedAddress::system(DB_METADATA);
        let mut view = View::new(&fork, addr);
        if let Some(saved_version) = view.get::<_, u8>(VERSION_NAME) {
            if saved_version != DB_VERSION {
                return Err(Error::new(format!(
                    "Database version doesn't match: actual {}, expected {}",
                    saved_version, DB_VERSION
                )));
            }

            return Ok(());
        } else {
            view.put(VERSION_NAME, DB_VERSION);
        }
    }
    db.merge(fork.into_patch())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{access::CopyAccessExt, ObjectHash, TemporaryDB};

    use std::{collections::HashSet, iter::FromIterator};

    #[test]
    fn readonly_indexes_are_timely_dropped() {
        let db = TemporaryDB::new();
        let fork = db.fork();
        fork.get_list("list").push(1_u64);
        {
            // The code without an additional scope must not compile.
            let _list = fork.readonly().get_list::<_, u64>("list");
        }
        fork.into_patch();
    }

    /// Asserts that a patch contains only the specified changes.
    fn check_patch<'a, I>(patch: &Patch, changes: I)
    where
        I: IntoIterator<Item = (&'a str, &'a [u8], Change)>,
    {
        let mut patch_set: HashSet<_> = HashSet::new();
        for (name, changes) in &patch.changes {
            for (key, value) in &changes.data {
                patch_set.insert((name.to_owned(), key.as_slice(), value.to_owned()));
            }
        }
        let expected_set: HashSet<_> = changes
            .into_iter()
            .map(|(name, key, change)| (ResolvedAddress::system(name), key, change))
            .collect();
        assert_eq!(patch_set, expected_set);
    }

    #[test]
    fn backup_data_is_correct() {
        let db = TemporaryDB::new();
        let fork = db.fork();
        {
            let mut view = View::new(&fork, "foo");
            view.put(&vec![], vec![2]);
        }
        let backup = db.merge_with_backup(fork.into_patch()).unwrap();
        check_patch(&backup, vec![("foo", &[] as &[u8], Change::Delete)]);
        let snapshot = db.snapshot();
        assert_eq!(snapshot.get(&"foo".into(), &[]), Some(vec![2]));

        let fork = db.fork();
        {
            let mut view = View::new(&fork, "foo");
            view.put(&vec![], vec![3]);
            let mut view = View::new(&fork, "bar");
            view.put(&vec![1], vec![4]);
            let mut view = View::new(&fork, "bar2");
            view.put(&vec![5], vec![6]);
        }
        let backup = db.merge_with_backup(fork.into_patch()).unwrap();
        check_patch(
            &backup,
            vec![
                ("bar2", &[5_u8] as &[u8], Change::Delete),
                ("bar", &[1], Change::Delete),
                ("foo", &[], Change::Put(vec![2])),
            ],
        );

        // Check that the old snapshot still corresponds to the same DB state.
        assert_eq!(snapshot.get(&"foo".into(), &[]), Some(vec![2]));
        let snapshot = db.snapshot();
        assert_eq!(snapshot.get(&"foo".into(), &[]), Some(vec![3]));
    }

    #[test]
    fn rollback_via_backup_patches() {
        let db = TemporaryDB::new();
        let fork = db.fork();
        {
            let mut view = View::new(&fork, "foo");
            view.put(&vec![], vec![2]);
        }
        db.merge(fork.into_patch()).unwrap();

        let fork = db.fork();
        {
            let mut view = View::new(&fork, "foo");
            view.put(&vec![], vec![3]);
            let mut view = View::new(&fork, "bar");
            view.put(&vec![1], vec![4]);
        }
        let backup = db.merge_with_backup(fork.into_patch()).unwrap();

        let snapshot = db.snapshot();
        assert_eq!(snapshot.get(&"foo".into(), &[]), Some(vec![3]));
        assert_eq!(backup.get(&"foo".into(), &[]), Some(vec![2]));
        assert_eq!(backup.get(&"bar".into(), &[1]), None);

        db.merge(backup).unwrap();
        let snapshot = db.snapshot();
        assert_eq!(snapshot.get(&"foo".into(), &[]), Some(vec![2]));
        assert_eq!(snapshot.get(&"bar".into(), &[1]), None);

        // Check that DB continues working as usual after a rollback.
        let fork = db.fork();
        {
            let mut view = View::new(&fork, "foo");
            view.put(&vec![], vec![4]);
            view.put(&vec![0, 0], vec![255]);
            let mut view = View::new(&fork, "bar");
            view.put(&vec![1], vec![253]);
        }
        let backup1 = db.merge_with_backup(fork.into_patch()).unwrap();
        let snapshot = db.snapshot();
        assert_eq!(snapshot.get(&"foo".into(), &[]), Some(vec![4]));
        assert_eq!(snapshot.get(&"foo".into(), &[0, 0]), Some(vec![255]));

        let fork = db.fork();
        {
            let mut view = View::new(&fork, "bar");
            view.put(&vec![1], vec![254]);
        }
        let backup2 = db.merge_with_backup(fork.into_patch()).unwrap();
        let snapshot = db.snapshot();
        assert_eq!(snapshot.get(&"foo".into(), &[]), Some(vec![4]));
        assert_eq!(snapshot.get(&"foo".into(), &[0, 0]), Some(vec![255]));
        assert_eq!(snapshot.get(&"bar".into(), &[1]), Some(vec![254]));

        // Check patches used as `Snapshot`s.
        assert_eq!(backup1.get(&"bar".into(), &[1]), None);
        assert_eq!(backup2.get(&"bar".into(), &[1]), Some(vec![253]));
        assert_eq!(backup1.get(&"foo".into(), &[]), Some(vec![2]));
        assert_eq!(backup2.get(&"foo".into(), &[]), Some(vec![4]));

        // Backups should be applied in the reverse order.
        db.merge(backup2).unwrap();
        db.merge(backup1).unwrap();
        let snapshot = db.snapshot();
        assert_eq!(snapshot.get(&"foo".into(), &[]), Some(vec![2]));
        assert_eq!(snapshot.get(&"foo".into(), &[0, 0]), None);
        assert_eq!(snapshot.get(&"bar".into(), &[1]), None);
    }

    #[test]
    fn backup_after_clearing_view() {
        let db = TemporaryDB::new();
        let fork = db.fork();
        {
            let mut view = View::new(&fork, "foo");
            view.put(&vec![], vec![1]);
            view.put(&vec![1], vec![2]);
        }
        db.merge(fork.into_patch()).unwrap();

        let fork = db.fork();
        {
            let mut view = View::new(&fork, "foo");
            view.clear();
            view.put(&vec![1], vec![3]);
            view.put(&vec![2], vec![4]);
        }
        let backup = db.merge_with_backup(fork.into_patch()).unwrap();
        assert_eq!(backup.get(&"foo".into(), &[]), Some(vec![1]));
        assert_eq!(backup.get(&"foo".into(), &[1]), Some(vec![2]));
        assert_eq!(backup.get(&"foo".into(), &[2]), None);
        db.merge(backup).unwrap();
        let snapshot = db.snapshot();
        assert_eq!(snapshot.get(&"foo".into(), &[]), Some(vec![1]));
        assert_eq!(snapshot.get(&"foo".into(), &[1]), Some(vec![2]));
        assert_eq!(snapshot.get(&"foo".into(), &[2]), None);
    }

    #[test]
    fn backup_reverting_index_creation() {
        let db = TemporaryDB::new();
        let fork = db.fork();
        fork.get_entry("foo").set(1_u32);
        db.merge(fork.into_patch()).unwrap();
        let fork = db.fork();
        fork.get_entry(("foo", &1_u8)).set(2_u32);
        let backup = db.merge_with_backup(fork.into_patch()).unwrap();
        assert!(backup.index_type(("foo", &1_u8)).is_none());
        assert!(backup.get_list::<_, u32>(("foo", &1_u8)).is_empty());
    }

    #[test]
    fn updated_addrs_are_efficiently_updated() {
        let db = TemporaryDB::new();
        let mut fork = db.fork();
        fork.get_proof_list("foo").push(1_u64);
        fork.get_proof_map("bar").put(&1_u64, 2_u64);
        fork.get_list("baz").push(3_u64);
        fork.flush();

        let changed_addrs: HashSet<_> = fork
            .patch
            .changed_aggregated_addrs
            .iter()
            .map(|(addr, ns)| {
                assert!(ns.is_empty());
                addr.name.as_str()
            })
            .collect();
        assert_eq!(changed_addrs, HashSet::from_iter(vec!["foo", "bar"]));

        let patch = fork.into_patch();
        assert!(patch.changed_aggregated_addrs.is_empty());
        let mut fork = Fork::from(patch);
        fork.get_list("baz").push(3_u64);
        fork.flush();
        assert!(fork.patch.changed_aggregated_addrs.is_empty());

        fork.get_proof_list("other_list").push(42_i32);
        fork.get_proof_map::<_, u64, u64>("bar").clear();
        fork.flush();

        let changed_addrs: HashSet<_> = fork
            .patch
            .changed_aggregated_addrs
            .iter()
            .map(|(addr, _)| addr.name.as_str())
            .collect();
        assert_eq!(changed_addrs, HashSet::from_iter(vec!["bar", "other_list"]));

        let patch = fork.into_patch();
        let aggregator = SystemSchema::new(&patch).state_aggregator();
        assert_eq!(
            aggregator.get(&"foo".to_owned()).unwrap(),
            patch.get_proof_list::<_, u64>("foo").object_hash()
        );
        assert_eq!(
            aggregator.get(&"bar".to_owned()).unwrap(),
            patch.get_proof_map::<_, u64, u64>("bar").object_hash()
        );
        assert_eq!(
            aggregator.get(&"other_list".to_owned()).unwrap(),
            patch.get_proof_list::<_, u64>("other_list").object_hash()
        );
    }

    #[test]
    fn borrows_from_owned_forks() {
        use crate::{access::AccessExt, Entry};

        let db = TemporaryDB::new();
        let fork = Rc::new(db.fork());
        let readonly: OwnedReadonlyFork = fork.as_readonly();
        // Modify an index via `fork`.
        fork.get_proof_list("list").extend(vec![1_i64, 2, 3]);
        // Check that if both `CopyAccessExt` and `AccessExt` traits are in scope, the correct one
        // is used for `Rc<Fork>`.
        let mut entry: Entry<Rc<Fork>, _> = fork.get_entry("entry");
        // Access the list via `readonly`.
        let list = readonly.get_proof_list::<_, i64>("list");
        assert_eq!(list.len(), 3);
        assert_eq!(list.get(1), Some(2));
        assert_eq!(list.iter_from(1).collect::<Vec<_>>(), vec![2, 3]);

        entry.set("!".to_owned());
        drop(entry);
        let entry = readonly.get_entry::<_, String>("entry");
        // Clone `readonly` access and get another `entry` instance.
        let other_readonly = readonly.clone();
        let other_entry = other_readonly.get_entry::<_, String>("entry");
        assert_eq!(entry.get().unwrap(), "!");
        assert_eq!(other_entry.get().unwrap(), "!");
    }

    #[test]
    fn concurrent_borrow_from_fork_and_readonly_fork() {
        let db = TemporaryDB::new();
        let fork = db.fork();
        // This entry is phantom.
        let _readonly_entry = fork.readonly().get_entry::<_, u32>(("entry", &1_u8));
        // This one is not phantom, but it has the same `ResolvedAddress` as the phantom entry.
        // Since phantom entries do not borrow changes from the `Fork`, this works fine.
        let _entry = fork.get_entry::<_, u32>("entry");
    }

    #[test]
    fn stale_read_from_phantom_index() {
        let db = TemporaryDB::new();
        let fork = db.fork();
        // Phantom entries are unusual in that they can lead to stale reads (sort of; we assume
        // that the database writer is smart enough to separate readonly and read-write parts
        // of the `Fork`, e.g., via `Prefixed` accesses).
        let phantom_entry = fork.readonly().get_entry::<_, u32>("entry");
        let mut entry = fork.get_entry::<_, u32>("entry");
        entry.set(1);
        assert_eq!(phantom_entry.get(), None);
    }

    #[test]
    #[should_panic(expected = "immutably while it's borrowed mutably")]
    fn borrow_from_readonly_fork_after_index_is_created() {
        let db = TemporaryDB::new();
        let fork = db.fork();
        let _entry = fork.get_entry::<_, u32>("entry");
        // Since the index is already created, this should lead to a panic.
        let _readonly_entry = fork.readonly().get_entry::<_, u32>("entry");
    }
}
