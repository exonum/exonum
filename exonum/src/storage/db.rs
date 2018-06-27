// Copyright 2018 The Exonum Team
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
    cmp::Ordering::{Equal, Greater, Less},
    collections::{
        btree_map::{BTreeMap, IntoIter as BtmIntoIter, Iter as BtmIter, Range},
        hash_map::{Entry as HmEntry, IntoIter as HmIntoIter, Iter as HmIter},
        Bound::{Included, Unbounded}, HashMap,
    },
    iter::{Iterator as StdIterator, Peekable},
};

use self::NextIterValue::*;
use super::Result;

/// Map containing changes with corresponding key.
#[derive(Debug, Clone)]
pub struct Changes {
    data: BTreeMap<Vec<u8>, Change>,
}

impl Changes {
    /// Creates a new empty `Changes` instance.
    fn new() -> Self {
        Self {
            data: BTreeMap::new(),
        }
    }

    /// Returns iterator over changes.
    pub fn iter(&self) -> BtmIter<Vec<u8>, Change> {
        self.data.iter()
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
#[derive(Debug, Clone)]
pub struct Patch {
    changes: HashMap<String, Changes>,
}

impl Patch {
    /// Creates a new empty `Patch` instance.
    fn new() -> Self {
        Self {
            changes: HashMap::new(),
        }
    }

    /// Returns changes for the given name.
    fn changes(&self, name: &str) -> Option<&Changes> {
        self.changes.get(name)
    }

    /// Returns a mutable reference to the changes corresponding to the `name`.
    fn changes_mut(&mut self, name: &str) -> Option<&mut Changes> {
        self.changes.get_mut(name)
    }

    /// Gets the corresponding entry in the map by the given name for in-place manipulation.
    fn changes_entry(&mut self, name: String) -> HmEntry<String, Changes> {
        self.changes.entry(name)
    }

    /// Inserts changes with the given name.
    fn insert_changes(&mut self, name: String, changes: Changes) {
        self.changes.insert(name, changes);
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
pub type Iter<'a> = Box<Iterator + 'a>;

/// An enum that represents a kind of change to some key in the storage.
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
/// [`remove_by_prefix`]) are performed to a fork, the subsequent reads act as if the changes
/// are applied to the database; in reality, these changes are accumulated in memory.
///
/// To apply changes to the database, you need to convert a `Fork` into a [`Patch`] using
/// [`into_patch`] and then atomically [`merge`] it into the database. If two
/// conflicting forks are merged into a database, this can lead to an inconsistent state. If you
/// need to consistently apply several sets of changes for the same data, the next fork should be
/// created after the previous fork has been merged.
///
/// `Fork` also supports checkpoints ([`checkpoint`], [`commit`] and
/// [`rollback`] methods), which allows to rollback some of the latest changes (e.g., after
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

// FIXME: make &mut Fork "unwind safe". (ECR-176)
pub struct Fork {
    snapshot: Box<Snapshot>,
    patch: Patch,
    changelog: Vec<(String, Vec<u8>, Option<Change>)>,
    logged: bool,
}

struct ForkIter<'a> {
    snapshot: Iter<'a>,
    changes: Option<Peekable<Range<'a, Vec<u8>, Change>>>,
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
/// use exonum::storage::{Database, MemoryDB};
///
/// // not declared as `mut db`!
/// let db: Box<Database> = Box::new(MemoryDB::new());
/// let mut fork = db.fork();
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
    fn snapshot(&self) -> Box<Snapshot>;

    /// Creates a new fork of the database from its current state.
    fn fork(&self) -> Fork {
        Fork {
            snapshot: self.snapshot(),
            patch: Patch::new(),
            changelog: Vec::new(),
            logged: false,
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
    /// will be returned. In case of an error the method guarantees no changes were applied to
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
    /// will be returned. In case of an error the method guarantees no changes were applied to
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
    fn iter<'a>(&'a self, name: &str, from: &[u8]) -> Iter<'a>;
}

/// A trait that defines streaming iterator over storage view entries.
pub trait Iterator {
    /// Advances the iterator and returns the next key and value.
    fn next(&mut self) -> Option<(&[u8], &[u8])>;

    /// Returns references to the current key and value of the iterator.
    fn peek(&mut self) -> Option<(&[u8], &[u8])>;
}

impl Snapshot for Fork {
    fn get(&self, name: &str, key: &[u8]) -> Option<Vec<u8>> {
        if let Some(changes) = self.patch.changes(name) {
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
        if let Some(changes) = self.patch.changes(name) {
            if let Some(change) = changes.data.get(key) {
                match *change {
                    Change::Put(..) => return true,
                    Change::Delete => return false,
                }
            }
        }
        self.snapshot.contains(name, key)
    }

    fn iter<'a>(&'a self, name: &str, from: &[u8]) -> Iter<'a> {
        let range = (Included(from), Unbounded);
        let changes = match self.patch.changes(name) {
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
    /// Creates a new checkpoint.
    ///
    /// # Panics
    ///
    /// Panics if another checkpoint was created before and has not been committed or rolled back.
    pub fn checkpoint(&mut self) {
        if self.logged {
            panic!("call checkpoint before rollback or commit");
        }
        self.logged = true;
    }

    /// Finalizes all changes after the latest checkpoint.
    ///
    /// # Panics
    ///
    /// Panics if there is no active checkpoint, or the latest checkpoint
    /// is already committed or rolled back.
    pub fn commit(&mut self) {
        if !self.logged {
            panic!("call commit before checkpoint");
        }
        self.changelog.clear();
        self.logged = false;
    }

    /// Rolls back all changes after the latest checkpoint.
    ///
    /// # Panics
    ///
    /// Panics if there is no active checkpoint, or the latest checkpoint
    /// is already committed or rolled back.
    pub fn rollback(&mut self) {
        if !self.logged {
            panic!("call rollback before checkpoint");
        }
        for (name, k, c) in self.changelog.drain(..).rev() {
            if let Some(changes) = self.patch.changes_mut(&name) {
                match c {
                    Some(change) => changes.data.insert(k, change),
                    None => changes.data.remove(&k),
                };
            }
        }
        self.logged = false;
    }

    /// Inserts a key-value pair into the fork.
    pub fn put(&mut self, name: &str, key: Vec<u8>, value: Vec<u8>) {
        let changes = self.patch
            .changes_entry(name.to_string())
            .or_insert_with(Changes::new);
        if self.logged {
            self.changelog.push((
                name.to_string(),
                key.clone(),
                changes.data.insert(key, Change::Put(value)),
            ));
        } else {
            changes.data.insert(key, Change::Put(value));
        }
    }

    /// Removes the key from the fork.
    pub fn remove(&mut self, name: &str, key: Vec<u8>) {
        let changes = self.patch
            .changes_entry(name.to_string())
            .or_insert_with(Changes::new);
        if self.logged {
            self.changelog.push((
                name.to_string(),
                key.clone(),
                changes.data.insert(key, Change::Delete),
            ));
        } else {
            changes.data.insert(key, Change::Delete);
        }
    }

    /// Removes all keys starting with the specified prefix from the column family
    /// with the given `name`.
    pub fn remove_by_prefix(&mut self, name: &str, prefix: Option<&Vec<u8>>) {
        let changes = self.patch
            .changes_entry(name.to_string())
            .or_insert_with(Changes::new);
        // Remove changes
        if let Some(prefix) = prefix {
            let keys = changes
                .data
                .range::<Vec<u8>, _>((Included(prefix), Unbounded))
                .map(|(k, _)| k.to_vec())
                .take_while(|k| k.starts_with(prefix))
                .collect::<Vec<_>>();
            for k in keys {
                changes.data.remove(&k);
            }
        } else {
            changes.data.clear();
        }
        // Remove from storage
        let mut iter = self.snapshot
            .iter(name, prefix.map_or(&[], |k| k.as_slice()));
        while let Some((k, ..)) = iter.next() {
            let change = changes.data.insert(k.to_vec(), Change::Delete);
            if self.logged {
                self.changelog.push((name.to_string(), k.to_vec(), change));
            }
        }
    }

    /// Converts the fork into `Patch`.
    pub fn into_patch(self) -> Patch {
        self.patch
    }

    /// Returns reference to the inner `Patch`.
    pub fn patch(&self) -> &Patch {
        &self.patch
    }

    /// Merges patch from another fork to this fork.
    ///
    /// If both forks have changed the same data, this can lead to an inconsistent state. Hence,
    /// this method is useful only if you are sure that forks interacted with different indices.
    ///
    /// # Panics
    ///
    /// Panics if checkpoint was created before and it was not committed or rolled back yet.
    pub fn merge(&mut self, patch: Patch) {
        if self.logged {
            panic!("call merge before commit or rollback");
        }

        for (name, changes) in patch {
            if let Some(in_changes) = self.patch.changes_mut(&name) {
                in_changes.data.extend(changes.into_iter());
                continue;
            }
            {
                self.patch.insert_changes(name.to_owned(), changes);
            }
        }
    }
}

impl AsRef<Snapshot> for Snapshot + 'static {
    fn as_ref(&self) -> &Snapshot {
        self
    }
}

impl AsRef<Snapshot> for Fork {
    fn as_ref(&self) -> &Snapshot {
        self
    }
}

impl ::std::fmt::Debug for Fork {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        write!(f, "Fork(..)")
    }
}

impl<'a> ForkIter<'a> {
    fn step(&mut self) -> NextIterValue {
        if let Some(ref mut changes) = self.changes {
            match changes.peek() {
                Some(&(k, change)) => match self.snapshot.peek() {
                    Some((key, ..)) => match *change {
                        Change::Put(..) => match k[..].cmp(key) {
                            Equal => Replaced,
                            Less => Inserted,
                            Greater => Stored,
                        },
                        Change::Delete => match k[..].cmp(key) {
                            Equal => Deleted,
                            Less => MissDeleted,
                            Greater => Stored,
                        },
                    },
                    None => match *change {
                        Change::Put(..) => Inserted,
                        Change::Delete => MissDeleted,
                    },
                },
                None => match self.snapshot.peek() {
                    Some(..) => Stored,
                    None => Finished,
                },
            }
        } else {
            match self.snapshot.peek() {
                Some(..) => Stored,
                None => Finished,
            }
        }
    }
}

impl<'a> Iterator for ForkIter<'a> {
    fn next(&mut self) -> Option<(&[u8], &[u8])> {
        loop {
            match self.step() {
                Stored => return self.snapshot.next(),
                Replaced => {
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
                Inserted => {
                    return self.changes.as_mut().unwrap().next().map(|(key, change)| {
                        (
                            key.as_slice(),
                            match *change {
                                Change::Put(ref value) => value.as_slice(),
                                Change::Delete => unreachable!(),
                            },
                        )
                    })
                }
                Deleted => {
                    self.changes.as_mut().unwrap().next();
                    self.snapshot.next();
                }
                MissDeleted => {
                    self.changes.as_mut().unwrap().next();
                }
                Finished => return None,
            }
        }
    }

    fn peek(&mut self) -> Option<(&[u8], &[u8])> {
        loop {
            match self.step() {
                Stored => return self.snapshot.peek(),
                Replaced | Inserted => {
                    return self.changes.as_mut().unwrap().peek().map(|&(key, change)| {
                        (
                            key.as_slice(),
                            match *change {
                                Change::Put(ref value) => value.as_slice(),
                                Change::Delete => unreachable!(),
                            },
                        )
                    })
                }
                Deleted => {
                    self.changes.as_mut().unwrap().next();
                    self.snapshot.next();
                }
                MissDeleted => {
                    self.changes.as_mut().unwrap().next();
                }
                Finished => return None,
            }
        }
    }
}

impl<T: Database> From<T> for Box<Database> {
    fn from(db: T) -> Self {
        Box::new(db) as Box<Database>
    }
}
