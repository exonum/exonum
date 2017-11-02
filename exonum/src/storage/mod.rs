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

//! A module that provides a high-level interface for working with stored data.
//!
//! # Database
//!
//! A [`Database`] is the underlying type for working with stored data. Internally `Database` is
//! a key-value storage with reading isolation and atomic writing. This is an embedded database,
//! which means that only the Exonum process can access it. You can interact with a `Database` from
//! different threads by cloning its instance.
//!
//! Exonum provides three types of database: [`LevelDB`], [`RocksDB`] and [`MemoryDB`]. By default
//! present all three types, but you can choose [`LevelDB`] or [`RocksDB`] with `--feature` param.
//! [`MemoryDB`] presents always. Also, you can make your own implementations of [`Database`] trait.
//! See its documentation for more.
//!
//! # Snapshot and Fork
//!
//! There is no way to directly interact with data in the database.
//!
//! If you need to read the data, you can create a [`Snapshot`] using method [`snapshot`][1]
//! of the `Database` instance. Snapshots provide a read isolation, so you are guaranteed to work
//! with consistent values even if the data in the database changes between reads. `Snapshot`
//! provides all the necessary methods for reading data from the database, so `&Storage`
//! is used as a storage view for creating read-only [indices] representation.
//!
//! If you need to make any changes to the data, you need to create a [`Fork`] using method
//! [`fork`][2] of the `Database` instance. As well as `Snapshot`, `Fork` provides read isolation
//! and also allows you to create a sequence of changes to the database that are specified
//! as [`Patch`]. Later you can atomically merge a patch into the database using method [`merge`].
//!
//! # `StorageKey` and `StorageValue` traits
//!
//! If you need to use your own data types as keys or values in storage, you need to implement
//! the [`StorageKey`] or [`StorageValue`] traits respectively. These traits are already
//! implemented for most standard types. See their documentation for more.
//!
//! # Indices
//!
//! Indexes are structures that represent high-level data collections that are stored in a database.
//! This concept is similar to tables in relational databases. All indexes are stored in a single
//! key-value table, but internally the keys of each index will be prefixed with the unique index
//! prefixes, so the data of the different indexes do not conflict with each other. The interfaces
//! of the indexes are similar to ordinary collections (like arrays, maps and sets), but their data
//! is located in persistent storage.
//!
//! Merklized indices support the ability to generate the cryptographic proofs about the inclusion
//! of data in them. Having such proof external client may verify locally that received data was
//! really authorized by the validators without having to replicate the entire blockchain contents.
//!
//! Exonum implements the following indices:
//!
//! - [`Entry`] is a specific index that stores only one value. Useful for global values, such as
//! configuration. Similar to combination of [`Box`] and [`Option`] from standard library.
//! - [`ListIndex`] is a list of items stored in sequential order. Similar to [`Vec`] collection
//! from standard library.
//! - [`SparseListIndex`] is a list of items stored in sequential order. Similar to `ListIndex`
//! but it may contain indexes without elements.
//! - [`MapIndex`] is a map of keys and values. Similar to [`BTreeMap`] collection from standard
//! library.
//! - [`ProofListIndex`] is a Merklized version of `ListIndex` which supports the cryptographic
//! proofs of existence and is implemented as a Merkle tree.
//! - [`ProofMapIndex`] is a Merklized version of `MapIndex` which supports the cryptographic
//! proofs of existence and is implemented as a Merkle Patricia tree.
//! - [`KeySetIndex`] and [`ValueSetIndex`] is a set of items, similar to [`BTreeSet`] and
//! [`HashSet`] from standard library.
//!
//! If necessary, it's possible to implement new specific types of indexes. To do this, you need
//! to create a wrapper over [`BaseIndex`] structure. See their documentation for more.
//!
//! [`Database`]: trait.Database.html
//! [`LevelDB`]: struct.LevelDB.html
//! [`RocksDB`]: struct.RocksDB.html
//! [`MemoryDB`]: struct.MemoryDB.html
//! [`Snapshot`]: trait.Snapshot.html
//! [`Fork`]: struct.Fork.html
//! [`Patch`]: type.Patch.html
//! [1]: trait.Database.html#tymethod.snapshot
//! [2]: trait.Database.html#method.fork
//! [`merge`]: trait.Database.html#tymethod.merge
//! [`StorageKey`]: trait.StorageKey.html
//! [`StorageValue`]: trait.StorageValue.html
//! [`Entry`]: struct.Entry.html
//! [`ListIndex`]: list_index/struct.ListIndex.html
//! [`SparseListIndex`]: sparse_list_index/struct.SparseListIndex.html
//! [`MapIndex`]: map_index/struct.MapIndex.html
//! [`ProofListIndex`]: proof_list_index/struct.ProofListIndex.html
//! [`ProofMapIndex`]: proof_map_index/struct.ProofMapIndex.html
//! [`KeySetIndex`]: key_set_index/struct.KeySetIndex.html
//! [`ValueSetIndex`]: value_set_index/struct.ValueSetIndex.html
//! [`BaseIndex`]: base_index/struct.BaseIndex.html
//! [indices]: #indices
//! [`Option`]: https://doc.rust-lang.org/std/option/enum.Option.html
//! [`Box`]: https://doc.rust-lang.org/std/boxed/struct.Box.html
//! [`Vec`]: https://doc.rust-lang.org/std/vec/struct.Vec.html
//! [`BTreeMap`]: https://doc.rust-lang.org/std/collections/struct.BTreeMap.html
//! [`BTreeSet`]: https://doc.rust-lang.org/std/collections/struct.BTreeSet.html
//! [`HashSet`]: https://doc.rust-lang.org/std/collections/struct.HashSet.html

pub use self::error::Error;
pub use self::db::{Database, Snapshot, Fork, Patch, Change, Iterator, Iter};

pub use self::rocksdb::{RocksDB, RocksDBOptions};
pub use self::memorydb::MemoryDB;

pub use self::keys::StorageKey;
pub use self::values::StorageValue;

pub use self::entry::Entry;

pub use self::base_index::{BaseIndex, BaseIndexIter};
pub use self::map_index::MapIndex;
pub use self::list_index::ListIndex;
pub use self::sparse_list_index::SparseListIndex;
pub use self::key_set_index::KeySetIndex;
pub use self::value_set_index::ValueSetIndex;
pub use self::proof_list_index::{ProofListIndex, ListProof};
pub use self::proof_map_index::{ProofMapIndex, MapProof};

/// A specialized `Result` type for I/O operations with storage.
pub type Result<T> = ::std::result::Result<T, Error>;

mod error;
mod db;
mod rocksdb;
mod memorydb;

mod keys;
mod values;

mod entry;

pub mod base_index;

pub mod map_index;
pub mod list_index;
pub mod sparse_list_index;
pub mod key_set_index;
pub mod value_set_index;
pub mod proof_list_index;
pub mod proof_map_index;

#[cfg(test)]
mod tests;
