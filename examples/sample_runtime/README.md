# Exonum: Sample runtime

[![Build status][travis-image]][travis-url]
[![Gitter][gitter-image]][gitter-url]

[travis-image]: https://travis-ci.com/exonum/exonum.svg?branch=master
[travis-url]: https://travis-ci.com/exonum/exonum
[gitter-image]: https://img.shields.io/gitter/room/exonum/exonum.svg?style=flat-square
[gitter-url]: https://gitter.im/exonum/exonum

Minimal [Exonum](https://github.com/exonum/exonum) blockchain example implementing
a simple runtime.

## Description

This example demonstrates the simplified version of [Rust runtime][rust-runtime],
able to deploy and run services in the Exonum blockchain.

The heart of this example is the `SampleRuntime` structure, which implements
`exonum::Runtime` trait.

`main` function contains the bootstrapping of the full Exonum node, which has
two runtimes:

- Rust runtime, full-fledged runtime for Rust services.
- Sample runtime, introduced by this example.

Later, a service is deployed and started for this Sample runtime, and several transactions
are executed in this service to demonstrate the interaction process.

## Running

Enter the example folder in terminal and then run the following:

```sh
cargo run
```

## License

Sample runtime is licensed under the Apache License (Version 2.0). See
[LICENSE](LICENSE) for details.

[rust-runtime]: ../../runtimes/rust
