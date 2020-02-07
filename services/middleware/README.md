# Exonum Middleware Service

[![Travis Build Status](https://img.shields.io/travis/exonum/exonum/master.svg?label=Linux%20Build)](https://travis-ci.com/exonum/exonum)
[![License: Apache-2.0](https://img.shields.io/github/license/exonum/exonum.svg)](https://github.com/exonum/exonum/blob/master/LICENSE)
![rust 1.36.0+ required](https://img.shields.io/badge/rust-1.36.0+-blue.svg?label=Required%20Rust)

`exonum-middleware-service` provides a collection of middleware
for [Exonum blockchain framework](https://exonum.com/),
allowing to compose Exonum transactions. For example, the service
allows to batch transactions in order to execute the batch atomically,
or to check the version of the service before performing a call to it.

Consult [the crate docs](https://docs.rs/exonum-middleware-service)
for more details about the service API.

## Functionality overview

### Transaction batching

Batching allows to atomically execute several transactions; if an error occurs
during execution, changes made by all transactions are rolled back. All
transactions in the batch are authorized in the same way as the batch itself.

### Checked call

Checked call is a way to ensure that the called service corresponds to a
specific artifact with an expected version range. Unlike alternatives (e.g.,
finding out this information via the `services` endpoint of the node HTTP API),
using checked calls is most failsafe; by design, it cannot suffer from [TOCTOU]
issues. It does impose a certain overhead on the execution, though.

## Usage

Include `exonum-middleware-service` as a dependency in your `Cargo.toml`:

```toml
[dependencies]
exonum = "1.0.0-rc.1"
exonum-middleware-service = "1.0.0-rc.1"
```

## License

`exonum-middleware-service` is licensed under the Apache License (Version 2.0).
See [LICENSE](LICENSE) for details.

[TOCTOU]: https://en.wikipedia.org/wiki/Time-of-check_to_time-of-use
