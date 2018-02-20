## Exonum Configuration Service

[![Travis Build Status](https://img.shields.io/travis/exonum/exonum/master.svg?label=Linux%20Build)](https://travis-ci.org/exonum/exonum)
![CircleCI Build Status](https://img.shields.io/circleci/project/github/exonum/exonum.svg?label=MacOS%20Build)
[![Docs.rs](https://docs.rs/exonum-configuration/badge.svg)](https://docs.rs/exonum-configuration)
[![License: Apache-2.0](https://img.shields.io/github/license/exonum/exonum.svg)](https://github.com/exonum/exonum/blob/master/LICENSE)
![rust 1.23+ required](https://img.shields.io/badge/rust-1.23+-blue.svg?label=Required%20Rust)

This crate implements a service for [Exonum] blockchain that provides
functionality of modifying the global configuration by the means of proposing a
new configuration and voting for proposed configurations among the validators.

- [Specification](https://exonum.com/doc/advanced/configuration-updater/)
- [Reference documentation](https://docs.rs/exonum-configuration)
- [Example code](examples/configuration.rs)
- [Testnet deploy and api usage tutorial](doc/testnet-api-tutorial.md)

## LICENSE

Exonum configuration service is licensed under the Apache License (Version 2.0).
See [LICENSE](LICENSE) for details.

[Exonum]: https://github.com/exonum/exonum
