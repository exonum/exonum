# exonum-testkit

[![Travis Build Status](https://img.shields.io/travis/exonum/exonum/master.svg?label=Linux%20Build)](https://travis-ci.org/exonum/exonum)
![CircleCI Build Status](https://img.shields.io/circleci/project/github/exonum/exonum.svg?label=MacOS%20Build)
[![Docs.rs](https://docs.rs/exonum_derive/badge.svg)](https://docs.rs/exonum_derive)
[![License: Apache-2.0](https://img.shields.io/github/license/exonum/exonum.svg)](https://github.com/exonum/exonum/blob/master/LICENSE)
![rust 1.23+ required](https://img.shields.io/badge/rust-1.23+-blue.svg?label=Required%20Rust)

Procedural macros for the Exonum blockchain framework.

## Usage

Add `exonum` and `exonum_derive` crates as dependencies, and use the within
your project as follows:

```rust
#[macro_use]
extern crate exonum;
#[macro_use]
extern crate exonum_derive;
```

### Deriving `MessageSet`

Declare an enum with variants corresponding to transaction types in your
service, and use `#[derive(MessageSet)]` attribute on it:

```rust
#[derive(Debug, MessageSet)]
#[exonum(service_id = "100")]
pub enum Transactions<'a> {
    CreateWallet {
        public_key: &'a PublicKey,
        name: &'a str,
    },

    Transfer {
        from: &'a PublicKey,
        to: &'a PublicKey,
        amount: u64,
        seed: u64,
    },
}
```

### Deriving `Message`

Declare a newtype wrapping `exonum::messages::RawMessage`, and use
`#[derive(Message)]` attribute on it:

```rust
#[derive(Clone, Message)]
#[exonum(payload = "Transactions")]
struct MyMessage(RawMessage);
```

## License

Licensed under the Apache License (Version 2.0). See [LICENSE](LICENSE) for details.
