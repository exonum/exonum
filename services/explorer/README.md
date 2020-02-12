# Exonum Explorer Service

[![Travis Build Status](https://img.shields.io/travis/exonum/exonum/master.svg?label=Linux%20Build)](https://travis-ci.com/exonum/exonum)
[![License: Apache-2.0](https://img.shields.io/github/license/exonum/exonum.svg)](https://github.com/exonum/exonum/blob/master/LICENSE)
![rust 1.36.0+ required](https://img.shields.io/badge/rust-1.36.0+-blue.svg?label=Required%20Rust)

`exonum-explorer-service` provides HTTP endpoints for exploring
Exonum blockchains.

This crate is distinct from the base [explorer][explorer] crate.
The base explorer provides Rust language APIs for retrieving info
from the blockchain, while this crate translates these APIs into
REST and WebSocket endpoints and packages this logic as an Exonum service.
Thus, this crate is useful if you want to provide the way for external apps
to query the blockchain info.

## Description

The explorer service does not define transactions, but it has several
REST / WebSocket endpoints allowing to retrieve information from the
blockchain in a structured way.

Usually, the explorer service should be instantiated at the blockchain start
with the default identifiers. There may be no more than one explorer service
on a blockchain; an attempt to create a second service instance will lead to
an error in the service constructor.

The API types necessary to interact with the service HTTP API are defined in
a separate crate, [`exonum-explorer`][explorer]. The base explorer provides
Rust language APIs for retrieving info from the blockchain, while this crate
translates these APIs into REST and WebSocket endpoints and packages this logic
as an Exonum service.

Thus, this crate is useful if you want to provide the way for external apps to
query the blockchain info.

## HTTP API

REST and WebSocket APIs of the service is documented in the crate docs.

## Usage

Include `exonum-explorer-service` as a dependency in your `Cargo.toml`:

```toml
[dependencies]
exonum = "1.0.0-rc.1"
exonum-explorer-service = "1.0.0-rc.1"
```

The explorer service should usually be initialized at the blockchain start
with the default identifiers. The service will refuse to instantiate
if an explorer service is already instantiated on the blockchain.

Consult [the crate docs](https://docs.rs/exonum-explorer-service)
for more details about the service API.

## License

`exonum-explorer-service` is licensed under the Apache License (Version 2.0).
See [LICENSE](LICENSE) for details.

[explorer]: https://crates.io/crates/exonum-explorer/
[api-module]: https://docs.rs/exonum-explorer-service/latest/exonum-explorer-service/api/index.html
[websocket-module]: https://docs.rs/exonum-explorer-service/latest/exonum-explorer-service/api/websocket/index.html
