# System API for Exonum node

[![Travis Build Status](https://img.shields.io/travis/exonum/exonum/master.svg?label=Linux%20Build)](https://travis-ci.com/exonum/exonum)
[![License: Apache-2.0](https://img.shields.io/github/license/exonum/exonum.svg)](https://github.com/exonum/exonum/blob/master/LICENSE)
![rust 1.36.0+ required](https://img.shields.io/badge/rust-1.36.0+-blue.svg?label=Required%20Rust)

Plugin extending HTTP API of the node to return information about node state.

## Usage

Include `exonum-system-api` as a dependency in your `Cargo.toml`:

```toml
[dependencies]
exonum-system-api = "0.13.0-rc.2"
```

`SystemApiPlugin` type, located at the root of the crate, should be used
as a node plugin during node creation.
Consult [the crate docs](https://docs.rs/exonum-system-api) for more details.

## License

`exonum-api` is licensed under the Apache License (Version 2.0).
See [LICENSE](LICENSE) for details.
