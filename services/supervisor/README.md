# exonum-supervisor

[![Travis Build Status](https://img.shields.io/travis/exonum/exonum/master.svg?label=Linux%20Build)](https://travis-ci.com/exonum/exonum)
[![License: Apache-2.0](https://img.shields.io/github/license/exonum/exonum.svg)](https://github.com/exonum/exonum/blob/master/LICENSE)
![rust 1.36.0+ required](https://img.shields.io/badge/rust-1.36.0+-blue.svg?label=Required%20Rust)

`exonum-supervisor` is a main service of the [Exonum blockchain framework](https://exonum.com/).
It is capable of deploying and starting new services,
and changing configuration of the started services.

## Usage

Include `exonum-supervisor` as a dependency in your `Cargo.toml`:

```toml
[dependencies]
exonum = "0.12.0"
exonum-supervisor = "0.12.0"
```

Consult [the crate docs](https://docs.rs/exonum-supervisor) for more details
about the service API.

## License

`exonum-supervisor` is licensed under the Apache License (Version 2.0).
See [LICENSE](LICENSE) for details.
