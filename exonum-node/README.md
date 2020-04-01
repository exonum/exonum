# Exonum Node Implementation

[![Travis Build Status](https://img.shields.io/travis/exonum/exonum/master.svg?label=Linux%20Build)](https://travis-ci.com/exonum/exonum)
[![License: Apache-2.0](https://img.shields.io/github/license/exonum/exonum.svg)](https://github.com/exonum/exonum/blob/master/LICENSE)
![rust 1.42.0+ required](https://img.shields.io/badge/rust-1.42.0+-blue.svg?label=Required%20Rust)

`exonum-node` provides a node implementation for the [Exonum](https://exonum.com/)
blockchain framework. Nodes form the blockchain network, in which they reach
consensus as to the latest blockchain state and process transactions coming
from external users. Besides transactions, nodes expose HTTP API of Exonum services
and node plugins.

## Usage

Include `exonum-node` as a dependency in your `Cargo.toml`:

```toml
[dependencies]
exonum = "1.0.0"
exonum-node = "1.0.0"
```

`exonum-node` provides relatively low-level (but more fine-grained) control
over node lifecycle. See [`exonum-cli`] for a more high-level alternative.

## License

`exonum-node` is licensed under the Apache License (Version 2.0).
See [LICENSE](LICENSE) for details.

[`exonum-cli`]: https://crates.io/crates/exonum-cli
