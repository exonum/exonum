# Exonum Time Oracle

[![Travis Build Status](https://img.shields.io/travis/exonum/exonum/master.svg?label=Linux%20Build)](https://travis-ci.com/exonum/exonum)
[![Docs.rs](https://docs.rs/exonum-time/badge.svg)](https://docs.rs/exonum-time)
[![License: Apache-2.0](https://img.shields.io/github/license/exonum/exonum.svg)](https://github.com/exonum/exonum/blob/master/LICENSE)
![rust 1.36.0+ required](https://img.shields.io/badge/rust-1.36.0+-blue.svg?label=Required%20Rust)

`exonum-time` is a time oracle service for [Exonum blockchain framework](https://exonum.com/).
This service allows to determine time,
import it from the external world to the blockchain
and keep its current value in the blockchain.

## Usage

Include `exonum-time` as a dependency in your `Cargo.toml`:

```toml
[dependencies]
exonum = "1.0.0-rc.1"
exonum-cli = "1.0.0-rc.1"
exonum-time = "1.0.0-rc.1"
```

## Examples

Examples of the node with `exonum-time` service, and service using
`exonum-time` service to obtain current time can be found in
the [examples](examples) folder:

- [node example]
- [service example]

## Further Reading

Consult the [service description in Exonum docs](https://exonum.com/doc/version/latest/advanced/time)
for a more high-level perspective, in particular, the design rationale
and the proof of correctness.

## Other languages support

- [Java Time Oracle](https://github.com/exonum/exonum-java-binding/tree/master/exonum-java-binding/time-oracle)

## License

`exonum-time` is licensed under the Apache License (Version 2.0).
See [LICENSE](LICENSE) for details.

[node example]: examples/exonum_time.rs
[service example]: examples/simple_service/main.rs
