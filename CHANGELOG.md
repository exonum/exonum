# Changelog

All notable changes to this project will be documented in this file.
The project adheres to [Semantic Versioning](http://semver.org/spec/v2.0.0.html).

## [Unreleased]

- `StorageValue` and `StorageKey` have been renamed to the `BinaryValue`
  and `BinaryKey`. (#4)

  - Added `to_bytes` method to the `BinaryValue` trait which doesn't consume
    original value instead of the `into_bytes`.
  - `BinaryKey::write` now returns total number of written bytes.
  - `CryptoHash` has been replaced by the `UniqueHash`.

- Changed the hash algorithm of the intermediate nodes in `ProofMapIndex`. (#1)

  `ProofPath` now uses compact binary representation in the `BranchNode`
  hash calculation.
  
  Binary representation is `|bits_len|bytes|`, where:

  - **bits_len** - total length of the given `ProofPath` in bits compressed
    by the `leb128` algorithm
  - **bytes** - non-null bytes of the given `ProofPath`, i.e. the first
    `(bits_len + 7) / 8` bytes.

- Exonum storage was been extracted to the separate crate `exonum-merkledb`.