# Cryptocurrency: Example Service

[![Build status][travis-image]][travis-url]
[![Gitter][gitter-image]][gitter-url]

[travis-image]: https://travis-ci.com/exonum/exonum.svg?branch=master
[travis-url]: https://travis-ci.com/exonum/exonum
[gitter-image]: https://img.shields.io/gitter/room/exonum/exonum.svg?style=flat-square
[gitter-url]: https://gitter.im/exonum/exonum

Minimal [Exonum](https://github.com/exonum/exonum) blockchain example implementing
a simple cryptocurrency.

See [the documentation](https://exonum.com/doc/version/latest/get-started/create-service)
for a detailed step-by-step guide how to approach this example.

## Prerequisites

To run this example you need to install [Rust](https://www.rust-lang.org/en-US/)
compiler and [third-party libraries](https://exonum.com/doc/version/latest/get-started/install/).

## Build & Run

### Blockchain Node

To build and run a single node use:

```sh
# clone the repository with blockchain node
git clone git@github.com:exonum/exonum.git
cd exonum/examples/cryptocurrency

# build and run a zero-configuration demo
cargo run --example demo
```

Now the node is listening to HTTP requests on <http://localhost:8000>.

### Sample Transactions & Read Requests

When node is launched, you can use transaction examples to check that it works properly.
A simplest way to do this is launching the [`test.sh`](examples/test.sh)
script in the **examples** directory (for \*NIX platforms), or [`test.ps1`](examples/test.ps1)
in the same directory (for Windows with PowerShell installed).
The script creates two wallets, performs a transfer
between them, and then verifies that the wallet status was correctly updated.

Alternatively, you may use command-line utilities, such as `curl`, to manually
POST transactions on [the transaction endpoint] and read data from wallet
endpoints (the [`wallets_info.sh`](examples/wallets_info.sh) script provides a
handy way to do this).

## License

Cryptocurrency is licensed under the Apache License (Version 2.0). See
[LICENSE](LICENSE) for details.

[the transaction endpoint]: http://127.0.0.1:8000/api/explorer/v1/transactions
