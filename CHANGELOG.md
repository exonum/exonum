# Changelog

All notable changes to this project will be documented in this file.
The project adheres to [Semantic Versioning](http://semver.org/spec/v2.0.0.html).

## [Unreleased]

- New API for getting and creating indexes. (#35)

  - Now indexes can be accessed via immutable references from `Snapshot` and
    mutable/immutable references from `Fork`.

  - Now it's possible to create root index from `Fork` using method
    `fork::create_root_object`.

  - `get` methods of `Fork` and `Snapshot` returns optional references to index.

- Updated `ProofMapIndex` data layout. (#31)

  Path to the root node in merkle patricia tree now has been stored in the index
  state.

- A crate-specific error type has been replaced by the `failure::Error`. (#27)

- Changed storage layout (#21)

  - Changed indexes metadata layout in the database.

  - Introduced a generic `IndexState` structure that can be used to store global
    index properties like total number of items.

- Changed `ProofMapIndex` hashing rules for branch nodes and root node.
  Branch nodes is hashing now with 0x04 prefix, root node with 0x03. (#20)

- Renamed method `merkle_root` of `ProofMapIndex` and `ProofListIndex` to
  `object_hash`. (#20)

- Several mutable indexes now can be create from immutable reference to `Fork` (#10)

- Relaxed trait bounds for the `ProofMapIndex` keys (#7)

  Now keys should just implement `BinaryKey` trait instead of the
  `ProofMapKey`, which will be ordered according to their binary
  representation, as in the `MapIndex`.

- Changed `ProofListIndex` hashing rules for leaf nodes and branch nodes according
  to the [certificate transparency](https://tools.ietf.org/html/rfc6962#section-2.1)
  specification. Leaf nodes contain hashes with 0x00 prefix, branch nodes - with
  0x01. (#2)

- `StorageValue` and `StorageKey` have been renamed to the `BinaryValue`
  and `BinaryKey`. (#4)

  - Added `to_bytes` method to the `BinaryValue` trait which doesn't consume
    original value instead of the `into_bytes`.
  - `BinaryKey::write` now returns total number of written bytes.
  - `CryptoHash` has been replaced by the `ObjectHash`.

- Changed the hash algorithm of the intermediate nodes in `ProofMapIndex`. (#1)

  `ProofPath` now uses compact binary representation in the `BranchNode`
  hash calculation.

  Binary representation is `|bits_len|bytes|`, where:

  - **bits_len** - total length of the given `ProofPath` in bits compressed
    by the `leb128` algorithm
  - **bytes** - non-null bytes of the given `ProofPath`, i.e. the first
    `(bits_len + 7) / 8` bytes.

- Exonum storage was been extracted to the separate crate `exonum-merkledb`.