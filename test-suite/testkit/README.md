# Testing Framework for Exonum Services

[![Travis Build Status](https://img.shields.io/travis/exonum/exonum/master.svg?label=Linux%20Build)](https://travis-ci.com/exonum/exonum)
[![Docs.rs](https://docs.rs/exonum-testkit/badge.svg)](https://docs.rs/exonum-testkit)
[![License: Apache-2.0](https://img.shields.io/github/license/exonum/exonum.svg)](https://github.com/exonum/exonum/blob/master/LICENSE)
![rust 1.36.0+ required](https://img.shields.io/badge/rust-1.36.0+-blue.svg?label=Required%20Rust)

Testkit for Exonum blockchain allows to test service operation.
Specifically, it allows to test transaction execution
and APIs in the synchronous environment (without consensus algorithm)
and in the same process as the test code.

## Usage

Add the following lines to the `Cargo.toml`:

```toml
[dev-dependencies]
exonum-testkit = "1.0.0-rc.1"
```

For more details, see [Exonum documentation][documentation].

## Examples

See the [**tests**](tests) and [**examples**](examples) folders for examples
of testing Exonum services with the testkit.

## License

Licensed under the Apache License (Version 2.0). See [LICENSE](LICENSE) for details.

[documentation]: https://exonum.com/doc/version/latest/advanced/service-testing/
