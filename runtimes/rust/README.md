# exonum-rust-runtime

[![Travis Build Status](https://img.shields.io/travis/exonum/exonum/master.svg?label=Linux%20Build)](https://travis-ci.com/exonum/exonum)
[![License: Apache-2.0](https://img.shields.io/github/license/exonum/exonum.svg)](https://github.com/exonum/exonum/blob/master/LICENSE)
![rust 1.36.0+ required](https://img.shields.io/badge/rust-1.36.0+-blue.svg?label=Required%20Rust)

The runtime is for running native services written in Rust.

This runtime is useful for writing services that exist throughout the whole life
of the blockchain. Good example of this kind of services is blockchain oracles.

Another good point to use Rust runtime are services with high performance
requirements because the runtime has lowest overhead.

In the Rust runtime a set of service artifacts that you may want to deploy is
static. The set is defined at the time of compilation. Once the set is created,
you can change it only by the node binary recompilation.

The Rust runtime does not provide any level of service isolation from the
operation system. Therefore, the security audit of the deployed artifacts
is up to the node administrators.

## Usage

You might look at one of these examples:

* [Cryptocurrency Service](../../examples/cryptocurrency/README.md)
* [Advanced Cryptocurrency Service](../../examples/cryptocurrency-advanced/backend/README.md)
* [Time Oracle](../../services/time/README.md)

## License

`exonum-rust-runtime` is licensed under the Apache License (Version 2.0).
See [LICENSE](LICENSE) for details.
