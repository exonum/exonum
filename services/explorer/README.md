# Exonum Explorer Service

[![Travis Build Status](https://img.shields.io/travis/exonum/exonum/master.svg?label=Linux%20Build)](https://travis-ci.com/exonum/exonum)
[![License: Apache-2.0](https://img.shields.io/github/license/exonum/exonum.svg)](https://github.com/exonum/exonum/blob/master/LICENSE)
![rust 1.36.0+ required](https://img.shields.io/badge/rust-1.36.0+-blue.svg?label=Required%20Rust)

`exonum-explorer-service` provides HTTP endpoints for exploring
the Exonum blockchain.

## Usage

Include `exonum-explorer-service` as a dependency in your `Cargo.toml`:

```toml
[dependencies]
exonum = "0.13.0-rc.2"
exonum-explorer-service = "0.13.0-rc.2"
```

The service should usually be included at the blockchain start
with the default identifiers. The service will refuse to instantiate
if an explorer service is already instantiated on the blockchain.

Consult [the crate docs](https://docs.rs/exonum-explorer-service)
for more details about the service API.

## License

`exonum-explorer-service` is licensed under the Apache License (Version 2.0).
See [LICENSE](LICENSE) for details.
