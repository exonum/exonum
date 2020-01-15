# Build scripts utility for Exonum

[![Travis Build Status](https://img.shields.io/travis/exonum/exonum/master.svg?label=Linux%20Build)](https://travis-ci.com/exonum/exonum)
[![License: Apache-2.0](https://img.shields.io/github/license/exonum/exonum.svg)](https://github.com/exonum/exonum/blob/master/LICENSE)
![rust 1.36.0+ required](https://img.shields.io/badge/rust-1.36.0+-blue.svg?label=Required%20Rust)

This crate simplifies writing build scripts for Exonum and Exonum services.

Since Protobuf is the Exonum default serialization format, build scripts
are mostly used to compile Protobuf files and generate corresponding code.
Generated code is used later by the Exonum core and services.

There are several predefined sets of protobuf sources available for use.
Currently presented sets:

- Crypto sources: all the necessary crypto types used in services
  and system proto-files. These types are Hash, PublicKey and Signature.
- Exonum sources: types used in core and in system services such
  as `Supervisor`.
- Common sources: types that can be used by various parts of Exonum.
- MerkleDB sources: types representing proofs of existence of element
  in database.

Consult [the crate docs](https://docs.rs/exonum-build) for more details.

## Examples

Sample `build.rs` using `exonum-build`:

```rust
use exonum_build::ProtobufGenerator;
use std::env;

fn main() {
    let current_dir = env::current_dir().expect("Failed to get current dir.");
    let protos = current_dir.join("src/proto");

    ProtobufGenerator::with_mod_name("protobuf_mod.rs")
        .with_input_dir("src/proto")
        .with_crypto()
        .generate();
}
```

## Usage

Include `exonum-build` as a dependency in your `Cargo.toml`:

```toml
[dependencies]
exonum-build = "0.13.0-rc.2"
```

## License

`exonum-build` is licensed under the Apache License (Version 2.0).
See [LICENSE](LICENSE) for details.
