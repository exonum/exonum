// Copyright 2019 The Exonum Team
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
    mem,
    ops::{Bound, Deref, DerefMut},
    rc::Rc,
    result::Result as StdResult,
};

use crate::{
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
    is_aggregated: bool,
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
        self.is_aggregated
    }

    pub fn clear(&mut self) {
        self.data.clear();
        self.is_cleared = true;
    }

    pub fn set_aggregation(&mut self, is_aggregated: bool) {
        self.is_aggregated = is_aggregated;
    }

    pub fn into_data(self) -> BTreeMap<Vec<u8>, Change> {
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
pub struct ChangesRef {
    inner: Rc<ViewChanges>,
}

impl Deref for ChangesRef {
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
            let changes = Rc::try_unwrap(changes).unwrap_or_else(|_| {
                panic!(
                    "changes are still immutably borrowed at address {:?}",
                    address
                );
            });

            if changes.is_aggregated {
                patch.changed_aggregated_addrs.insert(address.clone());
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
pub enum Change {
    /// Put the specified value into the storage for the corresponding key.
    Put(Vec<u8>),
    /// Delete a value from the storage for the corresponding key.
    Delete,
}

/// A combination of a database snapshot and a sequence of changes on top of it.
///
/// A `Fork` provides both immutable and mutable operations over the database. Like [`Snapshot`],
/// `Fork` provides read isolation. When mutable operations ([`put`], [`remove`] and
/// [`remove_by_prefix`]) are applied to a fork, the subsequent reads act as if the changes
/// are applied to the database; in reality, these changes are accumulated in memory.
///
/// To apply changes to the database, you need to convert a `Fork` into a [`Patch`] using
/// [`into_patch`] and then atomically [`merge`] it into the database. If two
/// conflicting forks are merged into a database, this can lead to an inconsistent state. If you
/// need to consistently apply several sets of changes to the same data, the next fork should be
/// created after the previous fork has been merged.
///
/// `Fork` also supports checkpoints ([`flush`] and
/// [`rollback`] methods), which allows rolling back some of the latest changes (e.g., after
/// a runtime error). Checkpoint is created automatically after calling the `flush` method.
///
/// `Fork` provides methods for both reading and writing data. Thus, `&Fork` is used
/// as a storage view for creating read-write indices representation.
///
/// **Note.** Unless stated otherwise, "key" in the method descriptions below refers
/// to a full key (a string column family name + key as an array of bytes within the family).
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
/// # use exonum_merkledb::{access::AccessExt, TemporaryDB, ListIndex, Database};
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
/// # use exonum_merkledb::{access::AccessExt, TemporaryDB, ListIndex, Database};
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
/// [`Snapshot`]: trait.Snapshot.html
/// [`put`]: #method.put
/// [`remove`]: #method.remove
/// [`remove_by_prefix`]: #method.remove_by_prefix
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

/// A set of serial changes that should be applied to a storage atomically.
///
/// This set can contain changes from multiple tables. When a block is added to
/// the blockchain, changes are first collected into a patch and then applied to
/// the storage.
#[derive(Debug)]
pub struct Patch {
    snapshot: Box<dyn Snapshot>,
    changes: HashMap<ResolvedAddress, ViewChanges>,
    /// Addresses of aggregated indexes that were changed within this patch. This information
    /// is used to update the state aggregator in `Fork::into_patch()`.
    changed_aggregated_addrs: HashSet<ResolvedAddress>,
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
/// and [`merge`] methods for indirect interaction. See [the module documentation](index.html)
/// for more details.
///
/// Note that `Database` effectively has [interior mutability][interior-mut];
/// `merge` and `merge_sync` methods take a shared reference to the database (`&self`)
/// rather than an exclusive one (`&mut self`). This means that the following code compiles:
///
/// ```
/// use exonum_merkledb::{access::AccessExt, Database, TemporaryDB};
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
/// [interior-mut]: https://doc.rust-lang.org/book/second-edition/ch15-05-interior-mutability.html
pub trait Database: Send + Sync + 'static {
    /// Creates a new snapshot of the database from its current state.
    fn snapshot(&self) -> Box<dyn Snapshot>;

    /// Creates a new fork of the database from its current state.
    fn fork(&self) -> Fork {
        Fork {
            patch: Patch {
                snapshot: self.snapshot(),
                changes: HashMap::new(),
                changed_aggregated_addrs: HashSet::new(),
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
    /// Merges a patch into the database and creates a rollback patch that reverses all the merged
    /// changes.
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
                    is_aggregated: changes.is_aggregated,
                },
            );
        }

        self.merge(patch)?;
        Ok(Patch {
            snapshot: self.snapshot(),
            changes: rev_changes,
            changed_aggregated_addrs,
        })
    }
}

impl<T: Database> DatabaseExt for T {}

/// A read-only snapshot of a storage backend.
///
/// A `Snapshot` instance is an immutable representation of a certain storage state.
/// It provides read isolation, so consistency is guaranteed even if the data in
/// the database changes between reads.
///
/// **Note.** Unless stated otherwise, "key" in the method descriptions below refers
/// to a full key (a string column family name + key as an array of bytes within the family).
pub trait Snapshot: Send + Sync + 'static {
    /// Returns a value corresponding to the specified key as a raw vector of bytes,
    /// or `None` if it does not exist.
    fn get(&self, name: &ResolvedAddress, key: &[u8]) -> Option<Vec<u8>>;

    /// Returns `true` if the snapshot contains a value for the specified key.
    ///
    /// Default implementation checks existence of the value using [`get`](#tymethod.get).
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

    pub fn finish_migration(&mut self, prefix: &str) {
        // Mutable `self` reference ensures that no indexes are instantiated in the client code.
        self.flush(); // Flushing is necessary to keep `self.patch` up to date.

        let prefix = format!("^{}", prefix);
        let removed_addrs = IndexesPool::new(&*self).remove_by_prefix(&prefix[1..]);
        for addr in removed_addrs {
            self.patch.changes.entry(addr).or_default().clear();
        }
        IndexesPool::new(&*self).finalize_migration_by_prefix(&prefix);
    }

    /// Rolls back all changes that were made after the latest execution
    /// of the `flush` method.
    pub fn rollback(&mut self) {
        self.working_patch = WorkingPatch::new();
    }

    /// Converts the fork into `Patch` consuming the fork instance.
    pub fn into_patch(mut self) -> Patch {
        self.flush();

        // Replacing `changed_aggregated_addrs` has a beneficial side-effect: if the patch
        // returned by this method is converted back to a `Fork`, we won't need to update
        // its state aggregator unless the *new* changes in the `Fork` concern aggregated indexes.
        let changed_aggregated_addrs =
            mem::replace(&mut self.patch.changed_aggregated_addrs, HashSet::new());
        let updated_entries = changed_aggregated_addrs
            .into_iter()
            .map(|addr| (addr.name.clone(), get_object_hash(&self.patch, addr)));

        SystemSchema::new(&self).update_state_aggregator(updated_entries);
        self.flush(); // flushes changes in the state aggregator
        self.patch
    }

    /// Returns a readonly wrapper around the fork. Indices created based on the readonly
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
    /// are sure that the fork and `patch` interacted with different indices.
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
/// # use exonum_merkledb::{access::AccessExt, Database, ReadonlyFork, TemporaryDB};
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
/// # use exonum_merkledb::{access::AccessExt, Database, ReadonlyFork, TemporaryDB};
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
    type Changes = ChangesRef;

    fn snapshot(&self) -> &dyn Snapshot {
        &self.0.patch
    }

    fn changes(&self, address: &ResolvedAddress) -> Self::Changes {
        ChangesRef {
            inner: self.0.working_patch.clone_view_changes(address),
        }
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
    use crate::{access::AccessExt, ObjectHash, TemporaryDB};

    use std::{collections::HashSet, iter::FromIterator};

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
            .map(|addr| addr.name.as_str())
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
            .map(|addr| addr.name.as_str())
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
        fork.get_entry("unrelated").set(1_u64);
        fork.get_entry("name1.unrelated").set(2_u64);

        // Start migration.
        fork.get_proof_list("^name.list").extend(vec![4_u64, 5]);
        fork.get_map("^name.map").put(&1_u64, 42_i32);
        fork.get_key_set("^name.new").insert(0_u8);
        fork.finish_migration("name.");

        check_indexes(&fork);
        // The newly migrated indexes are emptied.
        assert!(fork.get_proof_list::<_, u64>("^name.list").is_empty());

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

            assert_eq!(view.get_entry("unrelated").get(), Some(1_u64));
            assert_eq!(view.get_entry("name1.unrelated").get(), Some(2_u64));
            let set = view.get_value_set::<_, String>("name.removed");
            assert_eq!(set.iter().count(), 0);
        }

        let db = TemporaryDB::new();

        let fork = db.fork();
        fork.get_list("name.list").extend(vec![1_u32, 2, 3]);
        fork.get_map("name.map").put(&1_u64, "!".to_owned());
        fork.get_entry("unrelated").set(1_u64);
        fork.get_entry("name1.unrelated").set(2_u64);
        db.merge(fork.into_patch()).unwrap();

        let fork = db.fork();
        fork.get_proof_list("^name.list").extend(vec![4_u64, 5]);
        fork.get_map("^name.map").put(&1_u64, 42_i32);
        fork.get_key_set("^name.new").insert(0_u8);
        db.merge(fork.into_patch()).unwrap();

        let mut fork = db.fork();
        {
            let mut list = fork.get_proof_list::<_, u64>("^name.list");
            assert_eq!(list.len(), 2);
            list.push(6);
            list.push(7);
            assert_eq!(list.len(), 4);

            let mut map = fork.get_map::<_, u64, i32>("^name.map");
            map.clear();
            map.put(&2, 21);
            map.put(&3, 7);
        }
        fork.finish_migration("name.");

        check_indexes(&fork);
        db.merge(fork.into_patch()).unwrap();
        let snapshot = db.snapshot();
        check_indexes(&snapshot);
    }
}
