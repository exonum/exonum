# Blockchain Explorer Utils for Exonum

[![Travis Build Status](https://img.shields.io/travis/exonum/exonum/master.svg?label=Linux%20Build)](https://travis-ci.com/exonum/exonum)
[![License: Apache-2.0](https://img.shields.io/github/license/exonum/exonum.svg)](https://github.com/exonum/exonum/blob/master/LICENSE)
![rust 1.36.0+ required](https://img.shields.io/badge/rust-1.36.0+-blue.svg?label=Required%20Rust)

`exonum-explorer` provides explorer API
for the [Exonum blockchain framework](https://exonum.com/). For example,
it allows to request transactions from a block together with the execution
statuses, iterate over blocks, etc.

This crate is distinct from the [explorer *service*][explorer-service] crate.
While this crate provides Rust language APIs for retrieving info from the blockchain,
the explorer service translates these APIs into REST and WebSocket endpoints.
Correspondingly, this crate is primarily useful for Rust-language client apps.
Another use case is testing; the [testkit] relies on types in this crate
and re-exports it as the `explorer` module.

Consult [the crate docs](https://docs.rs/exonum-explorer)
and [examples](examples) for more details about the service API.

## Usage

Include `exonum-explorer` as a dependency in your `Cargo.toml`:

```toml
[dependencies]
exonum = "1.0.0-rc.1"
exonum-explorer = "1.0.0-rc.1"
```

## License

`exonum-explorer` is licensed under the Apache License (Version 2.0).
See [LICENSE](LICENSE) for details.

[explorer-service]: https://crates.io/crates/exonum-explorer-service/
[testkit]: https://crates.io/crate/exonum-testkit/
