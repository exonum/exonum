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

//! Interfaces to work with persisted blockchain data. The data can be *Merkelized*,
//! i.e., stored into authenticated data structures, which allow to prove presence or absence
//! of data with logarithmic overhead.
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
//! This crate provides two database types: [`RocksDB`] and [`TemporaryDB`].
//!
//! # Snapshot and Fork
//!
//! Snapshots and forks facilitate access to the database.
//!
//! If you need to read the data, you can create a [`Snapshot`] using the [`snapshot`][1] method
//! of the `Database` instance. Snapshots provide read isolation, so you are guaranteed to work
//! with consistent values even if the data in the database changes between reads. `Snapshot`
//! provides all the necessary methods for reading data from the database, so `&Snapshot`
//! is used as a storage view for creating a read-only representation of the [indexes](#indexes).
//!
//! If you need to make changes to the database, you need to create a [`Fork`] using
//! the [`fork`][2] method of the `Database`. Like `Snapshot`, `Fork` provides read isolation,
//! but also allows creating a sequence of changes to the database that are specified
//! as a [`Patch`]. A patch can be atomically [`merge`]d into a database. Different threads
//! may call `merge` concurrently.
//!
//! # `BinaryKey` and `BinaryValue` traits
//!
//! If you need to use your own data types as keys or values in the storage, you need to implement
//! the [`BinaryKey`] or [`BinaryValue`] traits respectively. These traits have already been
//! implemented for most standard types.
//!
//! # Indexes
//!
//! Indexes are structures representing data collections stored in the database.
//! This concept is similar to tables in relational databases. The interfaces
//! of the indexes are similar to ordinary collections (like arrays, maps and sets).
//!
//! Each index occupies a certain set of keys in a single column family of the [`Database`].
//! On the other hand, multiple indexes can be stored in the same column family, provided
//! that their key spaces do not intersect. Isolation is commonly achieved with the help
//! of [`Group`]s or keyed [`IndexAddress`]es.
//!
//! Merkelized indexes can generate cryptographic proofs about inclusion
//! of entries. Having such a proof, an external client may verify locally that the received data
//! was authorized by the blockchain validators, without having to replicate
//! the entire blockchain contents.
//!
//! This crate provides the following index types:
//!
//! - [`Entry`] is a specific index that stores only one value. Useful for global values, such as
//!   configuration. Similar to a combination of [`Box`] and [`Option`].
//! - [`ListIndex`] is a list of items stored in a sequential order. Similar to [`Vec`].
//! - [`SparseListIndex`] is a list of items stored in a sequential order. Similar to `ListIndex`,
//!   but may contain indexes without elements.
//! - [`MapIndex`] is a map of keys and values. Similar to [`BTreeMap`].
//! - [`ProofEntry`] is a Merkelized version of `Entry`.
//! - [`ProofListIndex`] is a Merkelized version of `ListIndex` that supports cryptographic
//!   proofs of existence and is implemented as a Merkle tree.
//! - [`ProofMapIndex`] is a Merkelized version of `MapIndex` that supports cryptographic
//!   proofs of existence and is implemented as a binary Merkle Patricia tree.
//! - [`KeySetIndex`] and [`ValueSetIndex`] are sets of items, similar to [`BTreeSet`] and
//!   [`HashSet`] accordingly.
//!
//! # State aggregation
//!
//! The database automatically aggregates its contents into a single `state_hash`, which commits
//! to the entire Merkelized database contents. For example, this is used in [Exonum] to achieve
//! consensus as to the database state.
//!
//! The `state_hash` of the database is the hash of [`state_aggregator`], a system `ProofMapIndex`
//! with keys being UTF-8 names of aggregated indexes, and values their hashes
//! as per [`ObjectHash`] implementation. An index is aggregated if and only if it satisfies
//! the following constraints:
//!
//! - Index has a matching type (`ProofListIndex`, `ProofMapIndex`, or `ProofEntry`)
//! - Index is not a part of a group, i.e., its address does not contain the `bytes` part
//!
//! The aggregation is automatically updated when a `Fork` is converted into a `Patch`.
//! Thus, `Snapshot`s (including `Patch`es!) are always consistent with respect
//! to the aggregated state; the index hashes in the `state_aggregator` match their actual values.
//! This is **not** the case for `Fork`s, in which `state_aggregator` may be stale.
//!
//! # Migrations
//!
//! The database [provides tooling](migration/index.html) for data migrations. With the help
//! of migration, it is possible to gradually accumulate changes to a set of indexes (including
//! across process restarts) and then atomically apply or discard these changes.
//!
//! [`Database`]: trait.Database.html
//! [`RocksDB`]: struct.RocksDB.html
//! [`TemporaryDB`]: struct.TemporaryDB.html
//! [`Snapshot`]: trait.Snapshot.html
//! [`Fork`]: struct.Fork.html
//! [`Patch`]: struct.Patch.html
//! [1]: trait.Database.html#tymethod.snapshot
//! [2]: trait.Database.html#method.fork
//! [`merge`]: trait.Database.html#tymethod.merge
//! [`BinaryKey`]: trait.BinaryKey.html
//! [`BinaryValue`]: trait.BinaryValue.html
//! [`Entry`]: indexes/entry/struct.Entry.html
//! [`ProofEntry`]: indexes/proof_entry/struct.ProofEntry.html
//! [`ListIndex`]: indexes/list/struct.ListIndex.html
//! [`SparseListIndex`]: indexes/sparse_list/struct.SparseListIndex.html
//! [`MapIndex`]: indexes/map/struct.MapIndex.html
//! [`ProofListIndex`]: indexes/proof_list/struct.ProofListIndex.html
//! [`ProofMapIndex`]: indexes/proof_map/struct.ProofMapIndex.html
//! [`KeySetIndex`]: indexes/key_set/struct.KeySetIndex.html
//! [`ValueSetIndex`]: indexes/value_set/struct.ValueSetIndex.html
//! [`ObjectHash`]: trait.ObjectHash.html
//! [doc:storage]: https://exonum.com/doc/architecture/storage
//! [`Option`]: https://doc.rust-lang.org/std/option/enum.Option.html
//! [`Box`]: https://doc.rust-lang.org/std/boxed/struct.Box.html
//! [`Vec`]: https://doc.rust-lang.org/std/vec/struct.Vec.html
//! [`BTreeMap`]: https://doc.rust-lang.org/std/collections/struct.BTreeMap.html
//! [`BTreeSet`]: https://doc.rust-lang.org/std/collections/struct.BTreeSet.html
//! [`HashSet`]: https://doc.rust-lang.org/std/collections/struct.HashSet.html
//! [`state_aggregator`]: struct.SystemSchema.html#method.state_aggregator
//! [`Group`]: indexes/group/struct.Group.html
//! [`IndexAddress`]: struct.IndexAddress.html
//! [Exonum]: https://exonum.com/

#![warn(
    missing_debug_implementations,
    unsafe_code,
    bare_trait_objects,
    missing_docs
)]
#![warn(clippy::pedantic)]
#![allow(
    // Next `cast_*` lints don't give alternatives.
    clippy::cast_possible_wrap, clippy::cast_possible_truncation, clippy::cast_sign_loss,
    // `filter(..).map(..)` often looks more shorter and readable.
    clippy::filter_map,
    // Next lints produce too much noise/false positives.
    clippy::module_name_repetitions, clippy::similar_names,
    // Variant name ends with the enum name. Similar behavior to similar_names.
    clippy::pub_enum_variant_names,
    // '... may panic' lints.
    clippy::indexing_slicing,
    clippy::use_self,
    clippy::default_trait_access,
)]

pub use self::{
    backends::{rocksdb::RocksDB, temporarydb::TemporaryDB},
    db::{Database, DatabaseExt, Fork, Iter, Iterator, Patch, ReadonlyFork, Snapshot},
    error::Error,
    hash::{root_hash, HashTag, ObjectHash, ValidationError},
    keys::BinaryKey,
    lazy::Lazy,
    options::DbOptions,
    values::BinaryValue,
    views::{AsReadonly, IndexAddress, IndexType, ResolvedAddress, SystemSchema},
};
// Workaround for 'Linked file at path {exonum_merkledb_path}/struct.ProofMapIndex.html
// does not exist!'
#[doc(no_inline)]
pub use self::indexes::{
    key_set::{self, KeySetIndex},
    list::{self, ListIndex},
    map::{self, MapIndex},
    proof_list::{self, ListProof, ProofListIndex},
    proof_map::{self, MapProof, ProofMapIndex, RawProofMapIndex},
    sparse_list::{self, SparseListIndex},
    value_set::{self, ValueSetIndex},
    Entry, Group, ProofEntry,
};

pub mod access;
pub mod indexes;
pub mod migration;
pub mod validation;

#[cfg(feature = "with-protobuf")]
pub mod proto;

#[cfg(feature = "with-protobuf")]
use exonum_proto::ProtobufConvert;

/// A specialized `Result` type for I/O operations with storage.
pub type Result<T> = std::result::Result<T, Error>;

#[macro_use]
mod macros;
mod backends;
mod db;
mod error;
mod hash;
mod keys;
mod lazy;
mod options;
mod values;
mod views;
