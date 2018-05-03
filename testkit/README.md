# exonum-testkit

[![Travis Build Status](https://img.shields.io/travis/exonum/exonum/master.svg?label=Linux%20Build)](https://travis-ci.org/exonum/exonum)
![CircleCI Build Status](https://img.shields.io/circleci/project/github/exonum/exonum.svg?label=MacOS%20Build)
[![Docs.rs](https://docs.rs/exonum-testkit/badge.svg)](https://docs.rs/exonum-testkit)
[![License: Apache-2.0](https://img.shields.io/github/license/exonum/exonum.svg)](https://github.com/exonum/exonum/blob/master/LICENSE)
![rust 1.23+ required](https://img.shields.io/badge/rust-1.23+-blue.svg?label=Required%20Rust)

Testkit for Exonum blockchain is a framework that allows to test operation
of the whole service. Specifically, it allows to test transaction execution
and APIs in the synchronous environment (without consensus algorithm)
and in the same system process.

## Usage

Just add the following line to the `Cargo.toml`:

```toml
[dev-dependencies]
exonum-testkit = "0.5.0"
```

[For more details, see Exonum documentation][documentation]

## Examples

See the [**tests**](tests) and [**examples**](examples) folders for examples
of building a service and then testing it with the testkit.

## License

Licensed under the Apache License (Version 2.0). See [LICENSE](LICENSE) for details.

[documentation]: https://exonum.com/doc/advanced/service-testing/
