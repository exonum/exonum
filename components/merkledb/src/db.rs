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
    cmp::Ordering::{Equal, Greater, Less},
    collections::{
        btree_map::{BTreeMap, IntoIter as BtmIntoIter, Iter as BtmIter},
        hash_map::{IntoIter as HmIntoIter, Iter as HmIter},
        Bound::{Included, Unbounded},
        HashMap,
    },
    fmt,
    iter::{Iterator as StdIterator, Peekable},
    mem,
    ops::{Deref, DerefMut},
    rc::Rc,
};

use crate::{
    views::{IndexAddress, RawAccess, ToReadonly, View},
    Error, Result,
};

/// Finds a prefix immediately following the supplied one.
pub fn next_prefix(prefix: &[u8]) -> Option<Vec<u8>> {
    let change_idx = prefix.iter().rposition(|&byte| byte < u8::max_value());
    change_idx.map(|idx| {
        let mut next_prefix = prefix.to_vec();
        next_prefix[idx] += 1;
        for byte in &mut next_prefix[(idx + 1)..] {
            *byte = 0;
        }
        next_prefix
    })
}

/// Removes all keys from the table that start with the specified prefix.
pub fn remove_keys_with_prefix<V>(table: &mut BTreeMap<Vec<u8>, V>, prefix: &[u8]) {
    if prefix.is_empty() {
        // If the prefix is empty, we can be more efficient by clearing
        // the entire changes in the patch.
        table.clear();
    } else {
        // Remove all keys starting from `prefix`.
        let mut tail = table.split_off(prefix);
        if let Some(next_prefix) = next_prefix(prefix) {
            tail = tail.split_off(&next_prefix);
            table.append(&mut tail);
        }
    }
}

/// Map containing changes with a corresponding key.
#[derive(Debug, Clone)]
pub struct Changes {
    data: BTreeMap<Vec<u8>, Change>,
    prefixes_to_remove: Vec<Vec<u8>>,
}

/// Map containing changes with a corresponding key.
#[derive(Debug, Clone)]
pub struct ViewChanges {
    pub(super) data: BTreeMap<Vec<u8>, Change>,
    empty: bool,
}

impl Changes {
    /// Creates a new empty `Changes` instance.
    fn new() -> Self {
        Self {
            data: BTreeMap::new(),
            prefixes_to_remove: Vec::new(),
        }
    }

    /// Returns an iterator over the changes.
    pub fn iter(&self) -> BtmIter<'_, Vec<u8>, Change> {
        self.data.iter()
    }

    /// Returns prefixes of keys that should be removed from the database.
    pub fn prefixes_to_remove(&self) -> &[Vec<u8>] {
        &self.prefixes_to_remove
    }
}

impl ViewChanges {
    fn new() -> Self {
        Self {
            data: BTreeMap::new(),
            empty: false,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.empty
    }

    pub fn clear(&mut self) {
        self.data.clear();
        self.empty = true;
    }
}

/// Iterator over the `Changes` data.
#[derive(Debug)]
pub struct ChangesIterator {
    inner: BtmIntoIter<Vec<u8>, Change>,
}

impl StdIterator for ChangesIterator {
    type Item = (Vec<u8>, Change);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

impl IntoIterator for Changes {
    type Item = (Vec<u8>, Change);
    type IntoIter = ChangesIterator;

    fn into_iter(self) -> Self::IntoIter {
        Self::IntoIter {
            inner: self.data.into_iter(),
        }
    }
}

type ChangesCell = Option<Rc<ViewChanges>>;

#[derive(Debug, Default)]
pub struct WorkingPatch {
    changes: RefCell<HashMap<IndexAddress, ChangesCell>>,
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
    key: IndexAddress,
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

    fn is_empty(&self) -> bool {
        self.changes.borrow().is_empty()
    }

    fn take_view_changes(&self, address: &IndexAddress) -> ChangesCell {
        let mut view_changes = {
            let mut changes = self.changes.borrow_mut();
            let view_changes = changes.get_mut(address).map(Option::take);
            view_changes.unwrap_or_else(|| {
                changes
                    .entry(address.clone())
                    .or_insert_with(|| Some(Rc::new(ViewChanges::new())))
                    .take()
            })
        };

        assert!(
            view_changes.is_some(),
            "multiple mutable borrows of an index at {:?}",
            address
        );
        assert!(
            Rc::get_mut(view_changes.as_mut().unwrap()).is_some(),
            "Attempting to borrow {:?} mutably while it's borrowed immutably",
            address
        );
        view_changes
    }

    fn clone_view_changes(&self, address: &IndexAddress) -> Rc<ViewChanges> {
        let mut changes = self.changes.borrow_mut();
        changes
            .entry(address.clone())
            .or_insert_with(|| Some(Rc::new(ViewChanges::new())))
            .as_ref()
            .unwrap_or_else(|| {
                panic!(
                    "Attempting to borrow {:?} immutably while it's borrowed mutably",
                    address
                );
            })
            .clone()
    }

    /// Returns an immutable reference to the changes corresponding to a certain index.
    ///
    /// # Panics
    ///
    /// If an index with the `address` is already mutably borrowed.
    pub fn changes(&self, address: &IndexAddress) -> ChangesRef {
        ChangesRef {
            inner: self.clone_view_changes(address),
        }
    }

    /// Returns a mutable reference to the changes corresponding to a certain index.
    ///
    /// # Panics
    ///
    /// If an index with the `address` is already borrowed either immutably or mutably.
    pub fn changes_mut(&self, address: &IndexAddress) -> ChangesMut<'_> {
        ChangesMut {
            changes: self.take_view_changes(address),
            key: address.clone(),
            parent: WorkingPatchRef::Borrowed(self),
        }
    }

    pub fn clear(&self, address: &IndexAddress) {
        let mut changes = self.changes.borrow_mut();
        let change = changes.entry(address.clone());
        change.and_modify(|v| *v = None);
    }

    // TODO: verify that this method updates `Change`s already in the `Patch` [ECR-2834]
    fn merge_into(self, patch: &mut Patch) {
        for (address, changes) in self.changes.into_inner() {
            let changes = changes.unwrap_or_else(|| {
                panic!("changes are still borrowed at address {:?}", address);
            });
            let changes = Rc::try_unwrap(changes).unwrap_or_else(|_| {
                panic!("changes are still borrowed at address {:?}", address);
            });

            let patch_changes = patch
                .changes
                .entry(address.name().to_owned())
                .or_insert_with(Changes::new);

            if changes.is_empty() {
                let prefix = address.bytes().map_or(vec![], |bytes| bytes.to_vec());
                remove_keys_with_prefix(&mut patch_changes.data, &prefix);

                // Remember the prefix to be dropped from the database
                patch_changes.prefixes_to_remove.push(prefix);
            }

            if address.bytes().is_none() {
                patch_changes.data.extend(changes.data);
            } else {
                patch_changes.data.extend(
                    changes
                        .data
                        .into_iter()
                        .map(|(key, value)| (address.keyed(&key).1.into_owned(), value)),
                );
            }
        }
    }
}

/// Iterator over the `Patch` data.
#[derive(Debug)]
pub struct PatchIterator {
    inner: HmIntoIter<String, Changes>,
}

impl StdIterator for PatchIterator {
    type Item = (String, Changes);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

impl IntoIterator for Patch {
    type Item = (String, Changes);
    type IntoIter = PatchIterator;

    fn into_iter(self) -> Self::IntoIter {
        Self::IntoIter {
            inner: self.changes.into_iter(),
        }
    }
}

/// A generalized iterator over the storage views.
pub type Iter<'a> = Box<dyn Iterator + 'a>;

/// An enum that represents a type of change made to some key in the storage.
#[derive(Debug, Clone, PartialEq)]
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
/// `Fork` implements the [`Snapshot`] trait and provides methods for both reading and
/// writing data. Thus, `&mut Fork` is used as a storage view for creating
/// read-write indices representation.
///
/// **Note.** Unless stated otherwise, "key" in the method descriptions below refers
/// to a full key (a string column family name + key as an array of bytes within the family).
///
/// # Borrow checking
///
/// It is possible to create only one instance of index with the specified name based on a
/// single fork. If an additional instance is requested, the code will panic in runtime.
/// Hence, obtaining indexes from a `Fork` functions similarly to [`RefCell::borrow_mut()`].
///
/// For example the code below will panic at runtime.
///
/// ```rust,should_panic
/// use exonum_merkledb::{access::AccessExt, TemporaryDB, ListIndex, Database};
/// let db = TemporaryDB::new();
/// let fork = db.fork();
///
/// let index = fork.as_ref().get_list::<_, u8>("index");
/// // This code will panic at runtime.
/// let index2 = fork.as_ref().get_list::<_, u8>("index");
/// ```
///
/// To enable immutable / shared references to indexes, you may use [`readonly`] method:
///
/// ```
/// use exonum_merkledb::{access::AccessExt, TemporaryDB, ListIndex, Database};
/// let db = TemporaryDB::new();
/// let fork = db.fork();
/// fork.as_ref().get_list::<_, u8>("index").extend(vec![1, 2, 3]);
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
/// Shared references work like `RefCell::borrow`; it is a runtime error to try to obtain
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
    changes: HashMap<String, Changes>,
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
///     let mut list = fork.as_ref().get_proof_list("list");
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
    fn get(&self, name: &str, key: &[u8]) -> Option<Vec<u8>>;

    /// Returns `true` if the snapshot contains a value for the specified key.
    ///
    /// Default implementation checks existence of the value using [`get`](#tymethod.get).
    fn contains(&self, name: &str, key: &[u8]) -> bool {
        self.get(name, key).is_some()
    }

    /// Returns an iterator over the entries of the snapshot in ascending order starting from
    /// the specified key. The iterator element type is `(&[u8], &[u8])`.
    fn iter(&self, name: &str, from: &[u8]) -> Iter<'_>;
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
    /// Return changes keyed by the index address.
    pub fn changes(&self) -> HashMap<String, Changes> {
        self.changes.clone()
    }

    /// Return an iterator over the underlying changes.
    pub fn iter(&self) -> HmIter<'_, String, Changes> {
        self.changes.iter()
    }
}

impl Snapshot for Patch {
    fn get(&self, name: &str, key: &[u8]) -> Option<Vec<u8>> {
        if let Some(changes) = self.changes.get(name) {
            if let Some(change) = changes.data.get(key) {
                match *change {
                    Change::Put(ref v) => return Some(v.clone()),
                    Change::Delete => return None,
                }
            }
        }
        self.snapshot.get(name, key)
    }

    fn contains(&self, name: &str, key: &[u8]) -> bool {
        if let Some(changes) = self.changes.get(name) {
            if let Some(change) = changes.data.get(key) {
                match *change {
                    Change::Put(..) => return true,
                    Change::Delete => return false,
                }
            }
        }
        self.snapshot.contains(name, key)
    }

    fn iter(&self, name: &str, from: &[u8]) -> Iter<'_> {
        let range = (Included(from), Unbounded);
        let changes = match self.changes.get(name) {
            Some(changes) => Some(changes.data.range::<[u8], _>(range).peekable()),
            None => None,
        };

        Box::new(ForkIter {
            snapshot: self.snapshot.iter(name, from),
            changes,
        })
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

    /// Rolls back all changes that were made after the latest execution
    /// of the `flush` method.
    pub fn rollback(&mut self) {
        self.working_patch = WorkingPatch::new();
    }

    /// Converts the fork into `Patch` consuming the fork instance.
    pub fn into_patch(mut self) -> Patch {
        self.flush();
        self.patch
    }

    /// Merges a patch from another fork to this fork.
    ///
    /// If both forks have changed the same data, this can lead to an inconsistent state. Hence,
    /// this method is useful only if you are sure that forks interacted with different indices.
    ///
    /// # Panics
    ///
    /// Panics if a target `Fork` contains unflushed changes.
    pub fn merge(&mut self, patch: Patch) {
        assert!(!self.is_dirty(), "cannot merge a dirty fork");

        for (name, changes) in patch {
            if let Some(in_changes) = self.patch.changes.get_mut(&name) {
                in_changes.data.extend(changes.into_iter());
                continue;
            }

            {
                self.patch.changes.insert(name.to_owned(), changes);
            }
        }
    }

    /// Checks if a fork has any unflushed changes.
    pub fn is_dirty(&self) -> bool {
        !self.working_patch.is_empty()
    }

    /// Patch containing current changes made in this fork.
    pub fn working_patch(&self) -> &WorkingPatch {
        &self.working_patch
    }

    /// Returns snapshot that also captures flushed changes in the fork,
    /// but does not capture unflushed changes.
    pub fn snapshot_without_unflushed_changes(&self) -> &dyn Snapshot {
        &self.patch
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

    fn changes(&self, address: &IndexAddress) -> Self::Changes {
        self.working_patch.changes_mut(address)
    }
}

impl RawAccess for Rc<Fork> {
    type Changes = ChangesMut<'static>;

    fn snapshot(&self) -> &dyn Snapshot {
        &self.patch
    }

    fn changes(&self, address: &IndexAddress) -> Self::Changes {
        let changes = self.working_patch.take_view_changes(address);
        ChangesMut {
            changes,
            key: address.clone(),
            parent: WorkingPatchRef::Owned(Self::clone(self)),
        }
    }
}

/// Readonly wrapper for a `Fork`.
#[derive(Debug, Clone, Copy)]
pub struct ReadonlyFork<'a>(&'a Fork);

impl<'a> ToReadonly for &'a Fork {
    type Readonly = ReadonlyFork<'a>;

    fn to_readonly(&self) -> Self::Readonly {
        ReadonlyFork(*self)
    }
}

impl<'a> RawAccess for ReadonlyFork<'a> {
    type Changes = ChangesRef;

    fn snapshot(&self) -> &dyn Snapshot {
        &self.0.patch
    }

    fn changes(&self, address: &IndexAddress) -> Self::Changes {
        ChangesRef {
            inner: self.0.working_patch.clone_view_changes(address),
        }
    }
}

impl AsRef<Fork> for Fork {
    fn as_ref(&self) -> &Fork {
        self
    }
}

impl AsRef<dyn Snapshot> for dyn Snapshot {
    fn as_ref(&self) -> &dyn Snapshot {
        self
    }
}

impl Snapshot for Box<dyn Snapshot> {
    fn get(&self, name: &str, key: &[u8]) -> Option<Vec<u8>> {
        self.as_ref().get(name, key)
    }

    fn contains(&self, name: &str, key: &[u8]) -> bool {
        self.as_ref().contains(name, key)
    }

    fn iter(&self, name: &str, from: &[u8]) -> Iter<'_> {
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
        let mut view = View::new(&fork, DB_METADATA);
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
