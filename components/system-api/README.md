# System API for Exonum node

[![Travis Build Status](https://img.shields.io/travis/exonum/exonum/master.svg?label=Linux%20Build)](https://travis-ci.com/exonum/exonum)
[![License: Apache-2.0](https://img.shields.io/github/license/exonum/exonum.svg)](https://github.com/exonum/exonum/blob/master/LICENSE)
![rust 1.36.0+ required](https://img.shields.io/badge/rust-1.36.0+-blue.svg?label=Required%20Rust)

Plugin extending HTTP API of the node to return information about node state.

## Description

The system API plugin provides information about the node state using REST interface.
The following info can be retrieved:

- Information about the current set of artifacts and services
- Network connectivity stats
- Version of Exonum / Rust that the node was compiled with

## HTTP API

REST API of the service is documented in the crate docs.

## Usage

Include `exonum-system-api` as a dependency in your `Cargo.toml`:

```toml
[dependencies]
exonum-system-api = "1.0.0-rc.1"
```

`SystemApiPlugin` type, located at the root of the crate, should be used
as a node plugin during node creation.
Consult [the crate docs](https://docs.rs/exonum-system-api) for more details.

## License

`exonum-system-api` is licensed under the Apache License (Version 2.0).
See [LICENSE](LICENSE) for details.
