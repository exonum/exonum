# Cryptocurrency Tutorial

[![Build status][travis-image]][travis-url]
[![Gitter][gitter-image]][gitter-url]

[travis-image]: https://img.shields.io/travis/exonum/cryptocurrency.svg?style=flat-square
[travis-url]: https://travis-ci.org/exonum/cryptocurrency
[gitter-image]: https://img.shields.io/gitter/room/exonum/exonum.svg?style=flat-square
[gitter-url]: https://gitter.im/exonum/exonum

Minimal Exonum blockchain example that uses the [Exonum](https://github.com/exonum/exonum) framework
to implement a simple cryptocurrency.

See [the documentation](https://exonum.com/doc/get-started/create-service)
for a detailed step-by-step guide how to approach this example.

## Prerequisites

To run this example you need to install [Rust](https://www.rust-lang.org/en-US/)
compiler and [third-party libraries](http://exonum.com/doc/get-started/install/).

## Build & Run

### Blockchain Node

To build and run a single node use:

```sh
# clone the repository with blockchain node
git clone git@github.com:exonum/cryptocurrency.git
cd cryptocurrency

# build and run
cargo run
```

Now the node is listening HTTP requests on `localhost:8000`.

### Sample Transactions & Read Requests

When node is launched, you can use transaction examples to check that it works properly.
A simplest way to do this is launching the [`test.sh`](examples/test.sh)
script in the **examples** directory. This script creates two wallets, performs a transfer
among them, and then verifies that the wallet status was correctly updated.

Alternatively, you may use command-line utilities, such as `curl`, to manually POST transactions
on [the transaction endpoint](http://127.0.0.1:8000/api/services/cryptocurrency/v1/wallets/transaction)
and read data from wallet endpoints (the [`wallets_info.sh`](examples/wallets_info.sh) script
provides a handy way to do this).

## License

Cryptocurrency is licensed under the Apache License (Version 2.0). See [LICENSE](LICENSE) for details.
