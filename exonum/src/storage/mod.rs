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

//! A module that provides interfaces to work with persisted blockchain data.
//!
//! See also [the documentation page on storage][doc:storage].
//!
//! # Database
//!
//! A [`Database`] is a container for data persistence. Internally, a `Database` is
//! a collection of named key-value stores (aka column families)
//! with reading isolation and atomic writes. The database is assumed to be embedded,
//! that is, the Exonum process has exclusive access to DB during blockchain operation.
//! You can interact with a `Database` from multiple threads by cloning its instance.
//!
//! Exonum provides two database types: [`RocksDB`] and [`MemoryDB`].
//!
//! # Snapshot and Fork
//!
//! Snapshots and forks facilitate access to the database.
//!
//! If you need to read the data, you can create a [`Snapshot`] using the [`snapshot`][1] method
//! of a `Database` instance. Snapshots provide read isolation, so you are guaranteed to work
//! with consistent values even if the data in the database changes between reads. `Snapshot`
//! provides all the necessary methods for reading data from the database, so `&Snapshot`
//! is used as a storage view for creating a read-only representation of [indices](#indices).
//!
//! If you need to make changes to the database, you need to create a [`Fork`] using
//! the [`fork`][2] method of a `Database`. Like `Snapshot`, `Fork` provides read isolation,
//! but also allows to create a sequence of changes to the database that are specified
//! as a [`Patch`]. A patch can be atomically [`merge`]d into a database. Different threads
//! may call `merge` concurrently.
//!
//! # `StorageKey` and `StorageValue` traits
//!
//! If you need to use your own data types as keys or values in storage, you need to implement
//! the [`StorageKey`] or [`StorageValue`] traits respectively. These traits are already
//! implemented for most standard types.
//!
//! # Indices
//!
//! Indices are structures representing data collections stored in a database.
//! This concept is similar to tables in relational databases. The interfaces
//! of the indices are similar to ordinary collections (like arrays, maps and sets).
//!
//! Each index occupies a certain set of keys in a single comlumn family of the [`Database`].
//! On the other hand, multiple indices can be stored in the same column family, provided
//! that their keyspaces do not intersect. Isolation is commonly achieved with the help
//! of prefixes; see `with_prefix` constructor in built-in index types.
//!
//! Merklized indices can generate cryptographic proofs about the inclusion
//! of entries. Having such a proof, an external client may verify locally that the received data
//! was authorized by the blockchain validators without having to replicate
//! the entire blockchain contents.
//!
//! Exonum provides the following index types:
//!
//! - [`Entry`] is a specific index that stores only one value. Useful for global values, such as
//!   configuration. Similar to a combination of [`Box`] and [`Option`].
//! - [`ListIndex`] is a list of items stored in sequential order. Similar to [`Vec`].
//! - [`SparseListIndex`] is a list of items stored in sequential order. Similar to `ListIndex`,
//!   but may contain indices without elements.
//! - [`MapIndex`] is a map of keys and values. Similar to [`BTreeMap`].
//! - [`ProofListIndex`] is a Merklized version of `ListIndex` that supports cryptographic
//!   proofs of existence and is implemented as a Merkle tree.
//! - [`ProofMapIndex`] is a Merklized version of `MapIndex` that supports cryptographic
//!   proofs of existence and is implemented as a binary Merkle Patricia tree.
//! - [`KeySetIndex`] and [`ValueSetIndex`] is a set of items, similar to [`BTreeSet`] and
//!   [`HashSet`].
//!
//! To implement a new index type, you should create a wrapper around [`BaseIndex`].
//!
//! [`Database`]: trait.Database.html
//! [`RocksDB`]: struct.RocksDB.html
//! [`MemoryDB`]: struct.MemoryDB.html
//! [`Snapshot`]: trait.Snapshot.html
//! [`Fork`]: struct.Fork.html
//! [`Patch`]: struct.Patch.html
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
//! [doc:storage]: https://exonum.com/doc/architecture/storage
//! [`Option`]: https://doc.rust-lang.org/std/option/enum.Option.html
//! [`Box`]: https://doc.rust-lang.org/std/boxed/struct.Box.html
//! [`Vec`]: https://doc.rust-lang.org/std/vec/struct.Vec.html
//! [`BTreeMap`]: https://doc.rust-lang.org/std/collections/struct.BTreeMap.html
//! [`BTreeSet`]: https://doc.rust-lang.org/std/collections/struct.BTreeSet.html
//! [`HashSet`]: https://doc.rust-lang.org/std/collections/struct.HashSet.html

pub use self::error::Error;
pub use self::db::{Database, Snapshot, Fork, Patch, PatchIterator, Change, Changes,
                   ChangesIterator, Iterator, Iter};

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
