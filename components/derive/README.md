# Procedural macros for Exonum

[![Travis Build Status](https://img.shields.io/travis/exonum/exonum/master.svg?label=Linux%20Build)](https://travis-ci.com/exonum/exonum)
[![License: Apache-2.0](https://img.shields.io/github/license/exonum/exonum.svg)](https://github.com/exonum/exonum/blob/master/LICENSE)
![rust 1.36.0+ required](https://img.shields.io/badge/rust-1.36.0+-blue.svg?label=Required%20Rust)

This crate provides several procedural macros for Exonum core and Exonum services.

Overview of presented macros:

- `BinaryValue`: derive macro for `BinaryValue` trait of MerkleDB.
  Depending on codec, the implementation may use `ProtobufConvert`
  trait as base (default), or `serde` traits using `bincode`.
- `ObjectHash`: derive macro for `ObjectHash` trait of MerkleDB.
  It can be used for any type that implements `BinaryValue` trait.
- `FromAccess`: derive macro for `FromAccess` trait for schemas of
  MerkleDB indexes.
- `ServiceDispatcher`: derive macro for generating dispatching mechanisms
  of Rust Exonum services.
- `ServiceFactory`: derive macro for generating factory mechanisms
  of Rust Exonum services.
- `exonum_interface`: attribute macro for transforming trait into interface
  of Rust Exonum service.
- `ExecutionFail`: derive macro similar to `failure::Fail`, implementing
  `ExecutionFail` trait for an enum.
- `RequireArtifact`: derive macro for `RequireArtifact` trait.

Consult [the crate docs](https://docs.rs/exonum-derive) for more details.

## Usage

Include `exonum-derive` as a dependency in your `Cargo.toml`:

```toml
[dependencies]
exonum-derive = "1.0.0-rc.1"
```

## License

`exonum-derive` is licensed under the Apache License (Version 2.0).
See [LICENSE](LICENSE) for details.
