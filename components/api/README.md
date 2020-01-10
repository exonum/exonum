# HTTP API engine for Exonum

[![Travis Build Status](https://img.shields.io/travis/exonum/exonum/master.svg?label=Linux%20Build)](https://travis-ci.com/exonum/exonum)
[![License: Apache-2.0](https://img.shields.io/github/license/exonum/exonum.svg)](https://github.com/exonum/exonum/blob/master/LICENSE)
![rust 1.36.0+ required](https://img.shields.io/badge/rust-1.36.0+-blue.svg?label=Required%20Rust)

`exonum-api` provides a high-level wrapper around the [`actix`] web server.
The wrapper is used in [Rust services][rust-runtime] and in plugins
for the Exonum node.

## Usage

Include `exonum-api` as a dependency in your `Cargo.toml`:

```toml
[dependencies]
exonum-api = "0.13.0-rc.2"
```

Consult [the crate docs](https://docs.rs/exonum-api) for more details.
Note that the crate rarely needs to be imported directly; it is re-exported
by the `exonum` crate.

## License

`exonum-api` is licensed under the Apache License (Version 2.0).
See [LICENSE](LICENSE) for details.

[`actix`]: https://crates.io/crates/actix
[rust-runtime]: ../../runtimes/rust
