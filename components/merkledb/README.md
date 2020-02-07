# Exonum MerkleDB

[![Travis Build Status](https://img.shields.io/travis/exonum/exonum/master.svg?label=Linux%20Build)](https://travis-ci.com/exonum/exonum)
[![Docs.rs](https://docs.rs/exonum-merkledb/badge.svg)](https://docs.rs/exonum-merkledb)
![rust 1.36.0+ required](https://img.shields.io/badge/rust-1.36.0+-blue.svg?label=Required%20Rust)
[![License: Apache-2.0](https://img.shields.io/github/license/exonum/exonum.svg)](https://github.com/exonum/exonum/components/merkledb/blob/master/LICENSE)

Exonum MerkleDB is a persistent storage implementation based on RocksDB
which provides APIs to work with merkelized data structures.

## Available Database Objects

- `Entry` is a specific index that stores only one value. Useful for global
  values, such as configuration. Similar to a combination of `Box` and
  `Option`.
- `ListIndex` is a list of items stored in a sequential order. Similar to
  `Vec`.
- `SparseListIndex` is a list of items stored in a sequential order. Similar
  to `ListIndex`, but may contain indices without elements.
- `MapIndex` is a map of keys and values. Similar to `BTreeMap`.
- `ProofListIndex` is a Merkelized version of `ListIndex` that supports
  cryptographic proofs of existence and is implemented as a Merkle tree.
- `ProofMapIndex` is a Merkelized version of `MapIndex` that supports cryptographic
  proofs of existence and is implemented as a binary Merkle Patricia tree.
- `KeySetIndex` and `ValueSetIndex` are sets of items, similar to `BTreeSet` and
  `HashSet` accordingly.

## Usage

Include `exonum-merkledb` as a dependency in your `Cargo.toml`:

```toml
[dependencies]
exonum-merkledb = "1.0.0-rc.1"

```

If you need only to read the data you can create `Snapshot`. Objects
created from `Snapshot` provide a read-only access to the storage data.
To modify data you need to create an object based on `Fork`.
`Fork` and `Snapshot` can be obtained from the `Database` object.
Currently only one database backend is supported - RockDB.

```rust
use std::path::Path;
use exonum_merkledb::{ProofListIndex, Database, ListProof, DbOptions, RocksDB};

let db_options = DbOptions::default();
let db = RocksDB::open(&Path::new("db"), &db_options).unwrap();
let list_name = "list";

// Read-only list
let snapshot = db.snapshot();
let list: ProofListIndex<_, u8> = ProofListIndex::new(list_name, &snapshot);

// Mutable list
let fork = db.fork();
let mut list: ProofListIndex<_, u8> = ProofListIndex::new(list_name, &fork);
```

After adding elements to the object you can obtain cryptographic proofs for their
existence or absence.

```rust
list.push(1);

assert_eq!(ListProof::Leaf(1), list.get_proof(0));

if let ListProof::Absent(_proof) = list.get_proof(1) {
    println!("Element with index 1 is absent")
}
```

## Further Reading

- [Blockchain example](examples/blockchain.rs)
- [MerkleDB description in Exonum docs](https://exonum.com/doc/version/latest/architecture/merkledb/)

## License

`exonum-merkledb` is licensed under the Apache License (Version 2.0).
See [LICENSE](LICENSE) for details.
