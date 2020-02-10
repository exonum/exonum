# Exonum Rust Runtime

[![Travis Build Status](https://img.shields.io/travis/exonum/exonum/master.svg?label=Linux%20Build)](https://travis-ci.com/exonum/exonum)
[![License: Apache-2.0](https://img.shields.io/github/license/exonum/exonum.svg)](https://github.com/exonum/exonum/blob/master/LICENSE)
![rust 1.36.0+ required](https://img.shields.io/badge/rust-1.36.0+-blue.svg?label=Required%20Rust)

The runtime is for running services written in Rust.

This runtime is useful for writing services that exist throughout the whole life
of the blockchain. A good example of this kind of services is blockchain oracles.

Another good point to use the Rust runtime are services with high performance
requirements because the runtime has lowest overhead.

In the Rust runtime, a set of service artifacts that you may want to deploy is
static. This set is defined at the time of compilation. Once the set is created,
you can change it only by recompiling the node binary.

The Rust runtime does not provide service isolation from the
operation system. Therefore, the security audit of the deployed artifacts
is up to the node administrators.

## Usage

You might look at one of these examples:

- [Cryptocurrency service][cryptocurrency]
- [Advanced cryptocurrency service][cryptocurrency-advanced]

...or these services developed along with the Exonum framework:

- [Supervisor](https://crates.io/crates/exonum-supervisor)
- [Time oracle](https://crates.io/crates/exonum-time)
- [Explorer](https://crates.io/crates/exonum-explorer-service)
- [Middleware](https://crates.io/crates/exonum-middleware-service)

## License

`exonum-rust-runtime` is licensed under the Apache License (Version 2.0).
See [LICENSE](LICENSE) for details.

[cryptocurrency]: https://github.com/exonum/exonum/blob/master/examples/cryptocurrency#readme
[cryptocurrency-advanced]: https://github.com/exonum/exonum/blob/master/examples/cryptocurrency-advanced#readme
