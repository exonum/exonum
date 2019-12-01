# exonum-time

[![Travis Build Status](https://img.shields.io/travis/exonum/exonum/master.svg?label=Linux%20Build)](https://travis-ci.com/exonum/exonum)
[![Docs.rs](https://docs.rs/exonum-time/badge.svg)](https://docs.rs/exonum-time)
[![License: Apache-2.0](https://img.shields.io/github/license/exonum/exonum.svg)](https://github.com/exonum/exonum/blob/master/LICENSE)
![rust 1.36.0+ required](https://img.shields.io/badge/rust-1.36.0+-blue.svg?label=Required%20Rust)

`exonum-time` is a time oracle service for [Exonum blockchain framework](https://exonum.com/).
This service allows to determine time, import it from the external world to the blockchain
and keep its current value in the blockchain.

## Usage

Include `exonum-time` as a dependency in your `Cargo.toml`:

```toml
[dependencies]
exonum = "0.12.0"
exonum-time = "0.12.0"
```

Consult [the crate docs](https://docs.rs/exonum-time) for more details about the service API.

Typical usage of the service boils down to importing the schema and calling its
`time()` or `validators_time()` methods.
For an example of usage, see the full implementation of the [service][service] using the time oracle.

## Further Reading

Consult the [service description in Exonum docs](https://exonum.com/doc/version/latest/advanced/time)
for a more high-level perspective, in particular, the design rationale and the proof of correctness.

## Other languages support

* [Java Time Oracle](https://github.com/exonum/exonum-java-binding/tree/master/exonum-java-binding/time-oracle)

## License

`exonum-time` is licensed under the Apache License (Version 2.0). See [LICENSE](LICENSE) for details.

[service]: examples/simple_service/main.rs
