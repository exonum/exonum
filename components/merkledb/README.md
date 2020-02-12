# Exonum MerkleDB

[![Travis Build Status](https://img.shields.io/travis/exonum/exonum/master.svg?label=Linux%20Build)](https://travis-ci.com/exonum/exonum)
[![Docs.rs](https://docs.rs/exonum-merkledb/badge.svg)](https://docs.rs/exonum-merkledb)
![rust 1.36.0+ required](https://img.shields.io/badge/rust-1.36.0+-blue.svg?label=Required%20Rust)
[![License: Apache-2.0](https://img.shields.io/github/license/exonum/exonum.svg)](https://github.com/exonum/exonum/components/merkledb/blob/master/LICENSE)

**MerkleDB** is a document-oriented persistent storage
which provides APIs to work with merkelized data structures.
Under the hood, MerkleDB uses RocksDB as a key-value storage.

## Features

- Supports list, map and set collections (aka *indexes*),
  as well as singular elements.
  Further, indexes can be organized into groups, allowing to create
  hierarchies of documents with arbitrary nesting.
- Automated state aggregation of top-level indexes into a single
  *state hash*, which reflects the entire database state.
- Ability to define data layouts in an intuitive, declarative format.
- Basic support of transactions: changes to the storage can be
  aggregated into a fork and then merged to the database atomically.
- Access control leveraging the Rust type system, allowing to precisely
  define access privileges for different actors.
- First-class support of long-running, fault-tolerant data migrations
  running concurrently with other I/O to the storage.

## Usage

Include `exonum-merkledb` as a dependency in your `Cargo.toml`:

```toml
[dependencies]
exonum-merkledb = "1.0.0-rc.1"
```

See [the description in Exonum docs][docs:merkledb] for a more detailed overview,
and the [examples](examples) for the examples of usage.

## License

`exonum-merkledb` is licensed under the Apache License (Version 2.0).
See [LICENSE](LICENSE) for details.

[docs:merkledb]: https://exonum.com/doc/version/latest/architecture/merkledb/
