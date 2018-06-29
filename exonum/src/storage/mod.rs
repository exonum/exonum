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

//! A module that provides interfaces to work with the persisted blockchain data.
//!
//! See also [the documentation page on storage][doc:storage].
//!
//! # Database
//!
//! A [`Database`] is a container for data persistence. Internally, a `Database` is
//! a collection of named key-value stores (aka column families)
//! with reading isolation and atomic writes. The database is assumed to be embedded,
//! that is, the Exonum process has exclusive access to the DB during blockchain operation.
//! You can interact with the `Database` from multiple threads by cloning its instance.
//!
//! Exonum provides two database types: [`RocksDB`] and [`MemoryDB`].
//!
//! # Snapshot and Fork
//!
//! Snapshots and forks facilitate access to the database.
//!
//! If you need to read the data, you can create a [`Snapshot`] using the [`snapshot`][1] method
//! of the `Database` instance. Snapshots provide read isolation, so you are guaranteed to work
//! with consistent values even if the data in the database changes between reads. `Snapshot`
//! provides all the necessary methods for reading data from the database, so `&Snapshot`
//! is used as a storage view for creating a read-only representation of the [indices](#indices).
//!
//! If you need to make changes to the database, you need to create a [`Fork`] using
//! the [`fork`][2] method of the `Database`. Like `Snapshot`, `Fork` provides read isolation,
//! but also allows to create a sequence of changes to the database that are specified
//! as a [`Patch`]. A patch can be atomically [`merge`]d into a database. Different threads
//! may call `merge` concurrently.
//!
//! # `StorageKey` and `StorageValue` traits
//!
//! If you need to use your own data types as keys or values in the storage, you need to implement
//! the [`StorageKey`] or [`StorageValue`] traits respectively. These traits are already
//! implemented for most standard types.
//!
//! # Indices
//!
//! Indices are structures representing data collections stored in the database.
//! This concept is similar to tables in relational databases. The interfaces
//! of the indices are similar to ordinary collections (like arrays, maps and sets).
//!
//! Each index occupies a certain set of keys in a single column family of the [`Database`].
//! On the other hand, multiple indices can be stored in the same column family, provided
//! that their key spaces do not intersect. Isolation is commonly achieved with the help
//! of column families; see `new_in_family` constructor in the built-in index types.
//!
//! Merkelized indices can generate cryptographic proofs about inclusion
//! of entries. Having such a proof, an external client may verify locally that the received data
//! was authorized by the blockchain validators without having to replicate
//! the entire blockchain contents.
//!
//! Exonum provides the following index types:
//!
//! - [`Entry`] is a specific index that stores only one value. Useful for global values, such as
//!   configuration. Similar to a combination of [`Box`] and [`Option`].
//! - [`ListIndex`] is a list of items stored in the sequential order. Similar to [`Vec`].
//! - [`SparseListIndex`] is a list of items stored in the sequential order. Similar to `ListIndex`,
//!   but may contain indices without elements.
//! - [`MapIndex`] is a map of keys and values. Similar to [`BTreeMap`].
//! - [`ProofListIndex`] is a Merkelized version of `ListIndex` that supports cryptographic
//!   proofs of existence and is implemented as a Merkle tree.
//! - [`ProofMapIndex`] is a Merkelized version of `MapIndex` that supports cryptographic
//!   proofs of existence and is implemented as a binary Merkle Patricia tree.
//! - [`KeySetIndex`] and [`ValueSetIndex`] is a set of items, similar to [`BTreeSet`] and
//!   [`HashSet`].
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
//! [doc:storage]: https://exonum.com/doc/architecture/storage
//! [`Option`]: https://doc.rust-lang.org/std/option/enum.Option.html
//! [`Box`]: https://doc.rust-lang.org/std/boxed/struct.Box.html
//! [`Vec`]: https://doc.rust-lang.org/std/vec/struct.Vec.html
//! [`BTreeMap`]: https://doc.rust-lang.org/std/collections/struct.BTreeMap.html
//! [`BTreeSet`]: https://doc.rust-lang.org/std/collections/struct.BTreeSet.html
//! [`HashSet`]: https://doc.rust-lang.org/std/collections/struct.HashSet.html

#[doc(no_inline)]
pub use self::proof_map_index::{HashedKey, MapProof, ProofMapIndex};
pub use self::{
    db::{
        Change, Changes, ChangesIterator, Database, Fork, Iter, Iterator, Patch, PatchIterator,
        Snapshot,
    },
    entry::Entry, error::Error, hash::UniqueHash, key_set_index::KeySetIndex, keys::StorageKey,
    list_index::ListIndex, map_index::MapIndex, memorydb::MemoryDB, options::DbOptions,
    proof_list_index::{ListProof, ProofListIndex}, rocksdb::RocksDB,
    sparse_list_index::SparseListIndex, value_set_index::ValueSetIndex, values::StorageValue,
};

/// A specialized `Result` type for I/O operations with storage.
pub type Result<T> = ::std::result::Result<T, Error>;

mod base_index;
mod db;
mod entry;
mod error;
mod hash;
mod indexes_metadata;
mod keys;
mod memorydb;
mod options;
mod rocksdb;
mod values;

pub mod key_set_index;
pub mod list_index;
pub mod map_index;
pub mod proof_list_index;
pub mod proof_map_index;
pub mod sparse_list_index;
pub mod value_set_index;

#[cfg(test)]
mod tests;
