// Copyright 2017 The Exonum Team
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

use std::collections::HashMap;
use std::collections::btree_map::{BTreeMap, Range};
use std::collections::Bound::*;
use std::cmp::Ordering::*;
use std::iter::Peekable;

use super::Result;

use self::NextIterValue::*;

/// A set of serial changes that should be applied to a storage atomically.
pub type Patch = HashMap<String, BTreeMap<Vec<u8>, Change>>;

/// A generalized iterator over the storage views.
pub type Iter<'a> = Box<Iterator + 'a>;

/// An enum that represents a kind of change to some key in storage.
#[derive(Debug, Clone)]
pub enum Change {
    /// Put the specified value into storage for a corresponding key.
    Put(Vec<u8>),
    /// Delete a value from storage for a corresponding key.
    Delete,
}

/// A combination of a database snapshot and a sequence of changes on top of it.
///
/// A `Fork` provides both immutable and mutable operations over database. As well as [`Snapshot`],
/// it provides read isolation. When mutable operations ([`put`], [`remove`] and
/// [`remove_by_prefix`] methods) are performed for fork, the data is presented as if these changes
/// are applied to the database. However, instead of applying changes directly to the database,
/// fork only accumulates these changes in memory.
///
/// To apply changes to the database, you need to convert a `Fork` into a [`Patch`] using method
/// [`into_patch`] and then atomically merge it into the database using method [`merge`]. If two
/// conflicting forks are merged into a database, this can lead to a non-consistent state. If you
/// need to consistently apply several sets of changes for the same data, the next fork should be
/// created after the previous fork has been merged.
///
/// `Fork` also supports a checkpoint mechanism (methods [`checkpoint`], [`commit`] and
/// [`rollback`]) that allows you to rollback some of the latest changes if for some reason you
/// can not provide a consistent state after the changes due to a runtime error.
///
/// `Fork` implements [`Snapshot`] trait and provides all the necessary methods for both reading and
/// writing data from the database, so `&mut Fork` is used as a storage view for creating
/// read-write indices representation.
///
/// [`Snapshot`]: trait.Snapshot.html
/// [`put`]: #method.put
/// [`remove`]: #method.remove
/// [`remove_by_prefix`]: #method.remove_by_prefix
/// [`Patch`]: type.Patch.html
/// [`into_patch`]: #method.into_patch
/// [`merge`]: trait.Database.html#tymethod.merge
/// [`checkpoint`]: #method.checkpoint
/// [`commit`]: #method.commit
/// [`rollback`]: #method.rollback
// FIXME: make &mut Fork "unwind safe" (ECR-176)
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

/// A trait that defines a low-level storage backend.
///
/// The trait `Database` requires to implement traits `Send` and `Sync` and should not be borrowed
/// data, so you can use method [`clone`] to get the references to the database for concurrent
/// usage.
///
/// There is no way to directly interact with data in the database.
///
/// If you only need to read the data, you can create a [`Snapshot`] using method [`snapshot`][1].
/// Snapshots provide a read isolation, so you are guaranteed to work with consistent values even
/// if the data in the database changes between reads.
///
/// If you need to make any changes to the data, you need to create a [`Fork`] using method
/// [`fork`][2]. As well as `Snapshot`, `Fork` provides read isolation and also allows you to create
/// a sequence of changes to the database that are specified as a [`Patch`]. Later you can
/// atomically merge a patch into the database using method [`merge`].
///
/// [`clone`]: #tymethod.fork
/// [`Snapshot`]: trait.Snapshot.html
/// [1]: #tymethod.snapshot
/// [`Fork`]: struct.Fork.html
/// [2]: #method.fork
/// [`Patch`]: type.Patch.html
/// [`merge`]: #tymethod.merge
pub trait Database: Send + Sync + 'static {
    /// Creates a new reference to the database as `Box<Database>`.
    fn clone(&self) -> Box<Database>;

    /// Creates a new snapshot of the database from its current state.
    ///
    /// See [`Snapshot`] documentation for more.
    /// [`Snapshot`]: trait.Snapshot.html
    fn snapshot(&self) -> Box<Snapshot>;

    /// Creates a new fork of the database from its current state.
    ///
    /// See [`Fork`] documentation for more.
    /// [`Fork`]: struct.Fork.html
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
    /// # Errors
    /// If this method encounters any form of I/O or other error during merging, an error variant
    /// will be returned. In case of an error the method guarantees no changes were applied to
    /// the database.
    fn merge(&mut self, patch: Patch) -> Result<()>;
}

/// A trait that defines a snapshot of storage backend.
///
/// `Snapshot` instance is immutable representation of storage state. It provides read isolation,
/// so using snapshot you are guaranteed to work with consistent values even if the data in
/// the database changes between reads.
///
/// `Snapshot` provides all the necessary methods for reading data from the database, so `&Storage`
/// is used as a storage view for creating read-only indices representation.
pub trait Snapshot: 'static {
    /// Returns a value as raw vector of bytes corresponding to the specified key
    /// or `None` if does not exist.
    fn get(&self, name: &str, key: &[u8]) -> Option<Vec<u8>>;

    /// Returns `true` if the snapshot contains a value for the specified key.
    ///
    /// Default implementation tries to read the value using method [`get`].
    /// [`get`]: #tymethod.get
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
        if let Some(changes) = self.patch.get(name) {
            if let Some(change) = changes.get(key) {
                match *change {
                    Change::Put(ref v) => return Some(v.clone()),
                    Change::Delete => return None,
                }
            }
        }
        self.snapshot.get(name, key)
    }

    fn contains(&self, name: &str, key: &[u8]) -> bool {
        if let Some(changes) = self.patch.get(name) {
            if let Some(change) = changes.get(key) {
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
        let changes = match self.patch.get(name) {
            Some(changes) => Some(changes.range::<[u8], _>(range).peekable()),
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
    /// Panics if another checkpoint was created before and it was not committed or rollbacked yet.
    pub fn checkpoint(&mut self) {
        if self.logged {
            panic!("call checkpoint before rollback or commit");
        }
        self.logged = true;
    }

    /// Finalizes all changes after the last checkpoint.
    ///
    /// # Panics
    ///
    /// Panics if checkpoint was not created before or last checkpoint is already committed or
    /// rollbacked.
    pub fn commit(&mut self) {
        if !self.logged {
            panic!("call commit before checkpoint");
        }
        self.changelog.clear();
        self.logged = false;
    }

    /// Rollbakcs all changes after the last checkpoint.
    ///
    /// # Panics
    ///
    /// Panics if checkpoint was not created before or last checkpoint is already committed or
    /// rollbacked.
    pub fn rollback(&mut self) {
        if !self.logged {
            panic!("call rollback before checkpoint");
        }
        for (name, k, c) in self.changelog.drain(..).rev() {
            if let Some(changes) = self.patch.get_mut(&name) {
                match c {
                    Some(change) => changes.insert(k, change),
                    None => changes.remove(&k),
                };
            }
        }
        self.logged = false;
    }

    /// Inserts the key-value pair into the fork with the given name `name`.
    pub fn put(&mut self, name: &str, key: Vec<u8>, value: Vec<u8>) {
        if !self.patch.contains_key(name) {
            self.patch.insert(name.to_string(), BTreeMap::new());
        }

        let changes = self.patch.get_mut(name).unwrap();

        if self.logged {
            self.changelog.push((
                name.to_string(),
                key.clone(),
                changes.insert(key, Change::Put(value)),
            ));
        } else {
            changes.insert(key, Change::Put(value));
        }
    }

    /// Removes the key from the fork with the given name `name`.
    pub fn remove(&mut self, name: &str, key: Vec<u8>) {
        if !self.patch.contains_key(name) {
            self.patch.insert(name.to_string(), BTreeMap::new());
        }

        let changes = self.patch.get_mut(name).unwrap();
        if self.logged {
            self.changelog.push((
                name.to_string(),
                key.clone(),
                changes.insert(key, Change::Delete),
            ));
        } else {
            changes.insert(key, Change::Delete);
        }
    }

    /// Removes all keys starting with the specified prefix from the fork with name `name`.
    pub fn remove_by_prefix(&mut self, name: &str, prefix: Option<&Vec<u8>>) {
        if !self.patch.contains_key(name) {
            self.patch.insert(name.to_string(), BTreeMap::new());
        }

        let changes = self.patch.get_mut(name).unwrap();
        // Remove changes
        if let Some(prefix) = prefix {
            let keys = changes
                .range::<Vec<u8>, _>((Included(prefix), Unbounded))
                .map(|(k, _)| k.to_vec())
                .take_while(|k| k.starts_with(prefix))
                .collect::<Vec<_>>();
            for k in keys {
                changes.remove(&k);
            }
        } else {
            changes.clear();
        }
        // Remove from storage
        let mut iter = self.snapshot.iter(
            name,
            prefix.map_or(&[], |k| k.as_slice()),
        );
        while let Some((k, ..)) = iter.next() {
            let change = changes.insert(k.to_vec(), Change::Delete);
            if self.logged {
                self.changelog.push((name.to_string(), k.to_vec(), change));
            }
        }
    }

    /// Converts the fork into `Patch`.
    pub fn into_patch(self) -> Patch {
        self.patch
    }

    /// Merges patch from another fork to this fork.
    ///
    /// If both forks have changed the same data, this can lead to a non-consistent state. So this
    /// method is useful only if you are sure that forks interacted with different indices.
    ///
    /// # Panics
    ///
    /// Panics if checkpoint was created before and it was not committed or rollbacked yet.
    pub fn merge(&mut self, patch: Patch) {
        if self.logged {
            panic!("call merge before commit or rollback");
        }

        for (name, changes) in patch {
            if let Some(in_changes) = self.patch.get_mut(&name) {
                in_changes.extend(changes);
                continue;
            }
            {
                self.patch.insert(name, changes);
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
                Some(&(k, change)) => {
                    match self.snapshot.peek() {
                        Some((key, ..)) => {
                            match *change {
                                Change::Put(..) => {
                                    match k[..].cmp(key) {
                                        Equal => Replaced,
                                        Less => Inserted,
                                        Greater => Stored,
                                    }
                                }
                                Change::Delete => {
                                    match k[..].cmp(key) {
                                        Equal => Deleted,
                                        Less => MissDeleted,
                                        Greater => Stored,
                                    }
                                }
                            }
                        }
                        None => {
                            match *change {
                                Change::Put(..) => Inserted,
                                Change::Delete => MissDeleted,
                            }
                        }
                    }
                }
                None => {
                    match self.snapshot.peek() {
                        Some(..) => Stored,
                        None => Finished,
                    }
                }
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
