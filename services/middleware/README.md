# exonum-middleware-service

[![Travis Build Status](https://img.shields.io/travis/exonum/exonum/master.svg?label=Linux%20Build)](https://travis-ci.com/exonum/exonum)
[![License: Apache-2.0](https://img.shields.io/github/license/exonum/exonum.svg)](https://github.com/exonum/exonum/blob/master/LICENSE)
![rust 1.36.0+ required](https://img.shields.io/badge/rust-1.36.0+-blue.svg?label=Required%20Rust)

`exonum-middleware-service` provides a collection of middleware
for [Exonum blockchain framework](https://exonum.com/),
allowing to compose Exonum transactions. For example, the service
allows to batch transactions in order to execute the batch atomically,
or to check the version of the service before performing a call to it.

## Usage

Include `exonum-middleware-service` as a dependency in your `Cargo.toml`:

```toml
[dependencies]
exonum = "0.13"
exonum-middleware-service = "0.13"
```

Consult [the crate docs](https://docs.rs/exonum-middleware-service)
for more details about the service API.

## License

`exonum-middleware-service` is licensed under the Apache License (Version 2.0).
See [LICENSE](LICENSE) for details.
