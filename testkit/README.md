# exonum-testkit

[![Version][crates-io-image]][crates-io-url]
[![Build status][travis-image]][travis-url]
[![Gitter][gitter-image]][gitter-url]

Testkit for Exonum blockchain is a framework that allows to test operation of the whole service.
Specifically, it allows to test transaction execution and APIs in the synchronous environment
(without consensus algorithm) and in the same system process.

## Usage

Just add the following line to the `Cargo.toml`:

```toml
[dev-dependencies]
exonum-testkit = "0.5.0"
```

[For more details, see Exonum documentation][documentation]

## Examples

See the [**tests**](tests) and [**examples**](examples) folders for examples of building a
service and then testing it with the testkit.

## License

Licensed under the Apache License (Version 2.0). See [LICENSE](LICENSE) for details.

[travis-image]: https://travis-ci.org/exonum/exonum.svg?branch=master
[travis-url]: https://travis-ci.org/exonum/exonum
[gitter-image]: https://img.shields.io/gitter/room/exonum/exonum.svg
[gitter-url]: https://gitter.im/exonum/exonum
[crates-io-image]: https://img.shields.io/crates/v/exonum-testkit.svg
[crates-io-url]: https://crates.io/crates/exonum-testkit
[documentation]: https://exonum.com/doc/advanced/service-testing/
