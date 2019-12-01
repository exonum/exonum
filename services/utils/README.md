# exonum-utils-service

[![Travis Build Status](https://img.shields.io/travis/exonum/exonum/master.svg?label=Linux%20Build)](https://travis-ci.com/exonum/exonum)
[![License: Apache-2.0](https://img.shields.io/github/license/exonum/exonum.svg)](https://github.com/exonum/exonum/blob/master/LICENSE)
![rust 1.36.0+ required](https://img.shields.io/badge/rust-1.36.0+-blue.svg?label=Required%20Rust)

`exonum-utils-service` provides a collection of utilities for [Exonum blockchain framework](https://exonum.com/),
based on the composability of Exonum transactions. For example, the service allows to batch transactions
in order to execute the batch atomically, or to check the version of the service before performing a call to it.

## Usage

Include `exonum-utils-service` as a dependency in your `Cargo.toml`:

```toml
[dependencies]
exonum = "0.12.0"
exonum-utils-service = "0.12.0"
```

Consult [the crate docs](https://docs.rs/exonum-utils-service) for more details about the service API.

## License

`exonum-utils-service` is licensed under the Apache License (Version 2.0).
See [LICENSE](LICENSE) for details.
