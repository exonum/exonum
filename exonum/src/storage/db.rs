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

use std::sync::Arc;
//use std::collections::btree_map::Range;
//use std::collections::Bound::*;
//use std::cmp::Ordering::*;
//use std::iter::Peekable;

//use self::NextIterValue::*;

///// A set of serial changes that should be applied to a storage atomically.
//pub type Patch = BTreeMap<Vec<u8>, Change>;

/// A generalized iterator over the storage views.
pub type Iter = Box<Iterator>;

///// An enum that represents a kind of change to some key in storage.
//#[derive(Debug, Clone)]
//pub enum Change {
//    /// Put the specified value into storage for a corresponding key.
//    Put(Vec<u8>),
//    /// Delete a value from storage for a corresponding key.
//    Delete,
//}

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
// FIXME: make &mut Fork "unwind safe"
//pub struct Fork {
//    snapshot: Box<Snapshot>,
//    changes: Patch,
//    changelog: Vec<(Vec<u8>, Option<Change>)>,
//    logged: bool,
//}
//
//struct ForkIter<'a> {
//    snapshot: Iter<'a>,
//    changes: Peekable<Range<'a, Vec<u8>, Change>>,
//}
//
//#[derive(Debug, PartialEq, Eq)]
//enum NextIterValue {
//    Stored,
//    Replaced,
//    Inserted,
//    Deleted,
//    MissDeleted,
//    Finished,
//}
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
pub trait Database: Send + Sync {
    /// Creates a new reference to the database as `Box<Database>`.
    fn clone(&self) -> Box<Database>;

    /// Creates a new snapshot of the database from its current state.
    ///
    /// See [`Snapshot`] documentation for more.
    /// [`Snapshot`]: trait.Snapshot.html
    fn snapshot(&self) -> Arc<View>;

    /// Creates a new transaction (fork) of the database
    fn fork(&self) -> Arc<View>;
}

/// A trait that defines a snapshot of storage backend.
///
/// `Snapshot` instance is immutable representation of storage state. It provides read isolation,
/// so using snapshot you are guaranteed to work with consistent values even if the data in
/// the database changes between reads.
///
/// `Snapshot` provides all the necessary methods for reading data from the database, so `&Storage`
/// is used as a storage view for creating read-only indices representation.
// TODO: should Snapshot be Send or Sync?
pub trait View: 'static {
    /// Returns a value as raw vector of bytes corresponding to the specified key
    /// or `None` if does not exist.
    fn get(&self, cf_name: &str, key: &[u8]) -> Option<Vec<u8>>;

    /// Returns `true` if the snapshot contains a value for the specified key.
    ///
    /// Default implementation tries to read the value using method [`get`].
    /// [`get`]: #tymethod.get
    fn contains(&self, cf_name: &str, key: &[u8]) -> bool {
        self.get(cf_name, key).is_some()
    }

    /// Returns an iterator over the entries of the snapshot in ascending order starting from
    /// the specified key. The iterator element type is `(&[u8], &[u8])`.
    fn iter(&self, cf_name: &str, from: Option<&[u8]>) -> Iter;

    /// Stores key and value into database for the corresponding column family
    fn put(&self, cf_name: &str, key: &[u8], value: &[u8]);

    /// Removes value by key
    fn delete(&self, cf_name: &str, key: &[u8]);

    /// Removes all values for column family `cf`
    fn clear(&self, cf_name: &str);

    /// Commits transaction's changes
    fn commit(&self);

    /// Rollbacks transaction's changes
    fn rollback(&self);

    /// Creates save point in transaction
    fn savepoint(&self);

    /// Rollbacks to save point
    fn rollback_to_savepoint(&self);
}

/// A trait that defines streaming iterator over storage view entries.
pub trait Iterator {
    /// Advances the iterator and returns the next key and value.
    fn next(&mut self) -> Option<(&[u8], &[u8])>;

    /// Returns references to the current key and value of the iterator.
    fn peek(&mut self) -> Option<(&[u8], &[u8])>;
}

impl ::std::fmt::Debug for View {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        write!(f, "View(..)")
    }
}
