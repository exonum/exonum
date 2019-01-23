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
    iter::{FromIterator, Iterator as StdIterator, Peekable},
    mem,
    ops::{Deref, DerefMut},
};

use crate::{
    views::{IndexAccess, IndexAddress},
    Result,
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
pub fn remove_prefix<V>(table: &mut BTreeMap<Vec<u8>, V>, prefix: &[u8]) {
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
    removed_prefixes: Vec<Vec<u8>>,
}

/// Map containing changes with a corresponding key.
#[derive(Debug, Clone)]
pub struct ViewChanges {
    pub(super) data: BTreeMap<Vec<u8>, Change>,
    clear: bool,
}

impl Changes {
    /// Creates a new empty `Changes` instance.
    fn new() -> Self {
        Self {
            data: BTreeMap::new(),
            removed_prefixes: Vec::new(),
        }
    }

    /// Returns an iterator over the changes.
    pub fn iter(&self) -> BtmIter<Vec<u8>, Change> {
        self.data.iter()
    }

    /// Returns prefixes of keys that should be removed from the database.
    pub fn removed_prefixes(&self) -> &[Vec<u8>] {
        &self.removed_prefixes
    }
}

impl ViewChanges {
    fn new() -> Self {
        Self {
            data: BTreeMap::new(),
            clear: false,
        }
    }

    pub fn is_cleared(&self) -> bool {
        self.clear
    }

    pub fn clear(&mut self) {
        self.data.clear();
        self.clear = true;
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

/// A set of serial changes that should be applied to a storage atomically.
///
/// This set can contain changes from multiple tables. When a block is added to
/// the blockchain, changes are first collected into a patch and then applied to
/// the storage.
#[derive(Debug, Clone)]
pub struct Patch {
    changes: HashMap<String, Changes>,
}

#[derive(Debug)]
pub struct WorkingPatch {
    changes: RefCell<HashMap<IndexAddress, Option<ViewChanges>>>,
}

/// `RefMut`, but dumber.
#[derive(Debug)]
pub struct ChangesRef<'a> {
    parent: &'a WorkingPatch,
    key: IndexAddress,
    changes: Option<ViewChanges>,
}

impl Deref for ChangesRef<'_> {
    type Target = ViewChanges;

    fn deref(&self) -> &ViewChanges {
        // `.unwrap()` is safe: `changes` can be equal to `None` only when
        // the instance is being dropped.
        self.changes.as_ref().unwrap()
    }
}

impl DerefMut for ChangesRef<'_> {
    fn deref_mut(&mut self) -> &mut ViewChanges {
        // `.unwrap()` is safe: `changes` can be equal to `None` only when
        // the instance is being dropped.
        self.changes.as_mut().unwrap()
    }
}

impl Drop for ChangesRef<'_> {
    fn drop(&mut self) {
        let mut change_map = self.parent.changes.borrow_mut();
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

    /// Returns a mutable reference to the changes corresponding to a certain index.
    fn changes_mut(&self, address: &IndexAddress) -> ChangesRef {
        let changes = self
            .changes
            .borrow_mut()
            .entry(address.clone())
            .or_insert_with(|| Some(ViewChanges::new()))
            .take();
        assert!(
            changes.is_some(),
            "multiple mutable borrows of an index at {:?}",
            address
        );

        ChangesRef {
            changes,
            key: address.clone(),
            parent: self,
        }
    }

    // TODO: verify that this method updates `Change`s already in the `Patch`
    fn merge_into(self, patch: &mut Patch) {
        for (address, changes) in self.changes.into_inner() {
            let changes = changes.unwrap_or_else(|| {
                panic!("changes are still borrowed at address {:?}", address);
            });

            let patch_changes = patch
                .changes
                .entry(address.name().to_owned())
                .or_insert_with(Changes::new);

            if changes.is_cleared() {
                let prefix = address.bytes().map_or(vec![], |bytes| bytes.to_vec());
                remove_prefix(&mut patch_changes.data, &prefix);

                // Remember the prefix to be dropped from the database
                patch_changes.removed_prefixes.push(prefix);
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

impl Patch {
    /// Creates a new empty `Patch` instance.
    fn new() -> Self {
        Self {
            changes: HashMap::new(),
        }
    }

    /// Returns iterator over changes.
    pub fn iter(&self) -> HmIter<String, Changes> {
        self.changes.iter()
    }

    /// Returns the number of changes.
    pub fn len(&self) -> usize {
        self.changes
            .iter()
            .fold(0, |acc, (_, changes)| acc + changes.data.len())
    }

    /// Returns `true` if this patch contains no changes and `false` otherwise.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Produces a patch that would reverse the effects of this patch for a given
    /// snapshot.
    pub fn undo(self, snapshot: &dyn Snapshot) -> Self {
        let mut rev_patch = Self::new();

        for (name, changes) in self {
            let rev_changes = BTreeMap::from_iter(changes.into_iter().map(|(key, ..)| {
                match snapshot.get(&name, &key) {
                    Some(value) => (key, Change::Put(value)),
                    None => (key, Change::Delete),
                }
            }));

            rev_patch.changes.insert(
                name,
                Changes {
                    data: rev_changes,
                    removed_prefixes: vec![],
                },
            );
        }

        rev_patch
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
/// `Fork` also supports checkpoints ([`checkpoint`], [`commit`] and
/// [`rollback`] methods), which allows rolling back some of the latest changes (e.g., after
/// a runtime error).
///
/// `Fork` implements the [`Snapshot`] trait and provides methods for both reading and
/// writing data. Thus, `&mut Fork` is used as a storage view for creating
/// read-write indices representation.
///
/// **Note.** Unless stated otherwise, "key" in the method descriptions below refers
/// to a full key (a string column family name + key as an array of bytes within the family).
///
/// [`Snapshot`]: trait.Snapshot.html
/// [`put`]: #method.put
/// [`remove`]: #method.remove
/// [`remove_by_prefix`]: #method.remove_by_prefix
/// [`Patch`]: struct.Patch.html
/// [`into_patch`]: #method.into_patch
/// [`merge`]: trait.Database.html#tymethod.merge
/// [`checkpoint`]: #method.checkpoint
/// [`commit`]: #method.commit
/// [`rollback`]: #method.rollback
pub struct Fork {
    flushed: FlushedFork,
    working_patch: WorkingPatch,
}

struct FlushedFork {
    snapshot: Box<dyn Snapshot>,
    patch: Patch,
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
/// use exonum_merkledb::{Database, TemporaryDB};
///
/// // not declared as `mut db`!
/// let db: Box<Database> = Box::new(TemporaryDB::new());
/// let fork = db.fork();
/// fork.put("index_name", vec![1, 2, 3], vec![123]);
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
            flushed: FlushedFork {
                snapshot: self.snapshot(),
                patch: Patch::new(),
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
pub trait Snapshot: 'static {
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
    fn iter(&self, name: &str, from: &[u8]) -> Iter;
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

impl Snapshot for FlushedFork {
    fn get(&self, name: &str, key: &[u8]) -> Option<Vec<u8>> {
        if let Some(changes) = self.patch.changes.get(name) {
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
        if let Some(changes) = self.patch.changes.get(name) {
            if let Some(change) = changes.data.get(key) {
                match *change {
                    Change::Put(..) => return true,
                    Change::Delete => return false,
                }
            }
        }
        self.snapshot.contains(name, key)
    }

    fn iter(&self, name: &str, from: &[u8]) -> Iter {
        let range = (Included(from), Unbounded);
        let changes = match self.patch.changes.get(name) {
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
    /// Finalizes all changes after the latest checkpoint.
    ///
    /// # Panics
    ///
    /// Panics if there is no active checkpoint, or the latest checkpoint
    /// is already committed or rolled back.
    pub fn flush(&mut self) {
        let working_patch = mem::replace(&mut self.working_patch, WorkingPatch::new());
        working_patch.merge_into(&mut self.flushed.patch);
    }

    /// Rolls back all changes after the latest checkpoint.
    ///
    /// # Panics
    ///
    /// Panics if there is no active checkpoint, or the latest checkpoint
    /// is already committed or rolled back.
    pub fn rollback(&mut self) {
        self.working_patch = WorkingPatch::new();
    }

    /// Converts the fork into `Patch` consuming the fork instance.
    pub fn into_patch(mut self) -> Patch {
        self.flush();
        self.flushed.patch
    }

    /// Merges a patch from another fork to this fork.
    ///
    /// If both forks have changed the same data, this can lead to an inconsistent state. Hence,
    /// this method is useful only if you are sure that forks interacted with different indices.
    ///
    /// # Panics
    ///
    /// Panics if a checkpoint has been created before and has not been committed
    /// or rolled back yet.
    pub fn merge(&mut self, patch: Patch) {
        assert!(!self.is_dirty(), "cannot merge a dirty fork");

        for (name, changes) in patch {
            if let Some(in_changes) = self.flushed.patch.changes.get_mut(&name) {
                in_changes.data.extend(changes.into_iter());
                continue;
            }

            {
                self.flushed.patch.changes.insert(name.to_owned(), changes);
            }
        }
    }

    /// Checks if a fork has any unflushed changes.
    pub fn is_dirty(&self) -> bool {
        !self.working_patch.is_empty()
    }
}

impl<'a> IndexAccess for &'a Fork {
    type Changes = ChangesRef<'a>;

    fn snapshot(&self) -> &dyn Snapshot {
        &self.flushed
    }

    fn changes(&self, address: &IndexAddress) -> Self::Changes {
        self.working_patch.changes_mut(address)
    }
}

// TODO: remove
impl AsRef<dyn Snapshot> for dyn Snapshot + 'static {
    fn as_ref(&self) -> &dyn Snapshot {
        self
    }
}

impl ::std::fmt::Debug for Fork {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        write!(f, "Fork(..)")
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

#[cfg_attr(feature = "cargo-clippy", allow(clippy::use_self))]
impl<T: Database> From<T> for Box<dyn Database> {
    fn from(db: T) -> Self {
        Box::new(db) as Self
    }
}
