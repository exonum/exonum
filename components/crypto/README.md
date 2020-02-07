# Cryptography primitives for Exonum

[![Travis Build Status](https://img.shields.io/travis/exonum/exonum/master.svg?label=Linux%20Build)](https://travis-ci.com/exonum/exonum)
[![License: Apache-2.0](https://img.shields.io/github/license/exonum/exonum.svg)](https://github.com/exonum/exonum/blob/master/LICENSE)
![rust 1.36.0+ required](https://img.shields.io/badge/rust-1.36.0+-blue.svg?label=Required%20Rust)

`exonum-crypto` provides a high-level API for work with various cryptography tasks.

Capabilities of `exonum-crypto` include:

- Calculating the hash of data;
- Generating key pairs for work with digital signatures;
- Creating and verifying of digital signatures.

The main backend for `exonum-crypto` is `sodiumoxide`, and the used algorithms are:

- SHA-256 for hashing.
- Ed25519 for digital signatures.

Consult [the crate docs](https://docs.rs/exonum-crypto) for more details.

## Examples

Signing data and verifying the signature:

```rust
exonum_crypto::init();
let (public_key, secret_key) = exonum_crypto::gen_keypair();
let data = [1, 2, 3];
let signature = exonum_crypto::sign(&data, &secret_key);
assert!(exonum_crypto::verify(&signature, &data, &public_key));
```

Hashing fixed amount of data:

```rust
exonum_crypto::init();
let data = [1, 2, 3];
let hash = exonum_crypto::hash(&data);
```

Hashing data by chunks:

```rust
use exonum_crypto::HashStream;

exonum_crypto::init();
let data: Vec<[u8; 5]> = vec![[1, 2, 3, 4, 5], [6, 7, 8, 9, 10]];
let mut hash_stream = HashStream::new();
for chunk in data {
    hash_stream = hash_stream.update(&chunk);
}
let _ = hash_stream.hash();
```

## Usage

Include `exonum-crypto` as a dependency in your `Cargo.toml`:

```toml
[dependencies]
exonum-crypto = "1.0.0-rc.1"
```

## License

`exonum-crypto` is licensed under the Apache License (Version 2.0).
See [LICENSE](LICENSE) for details.
