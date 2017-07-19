# Cryptocurrency (demo)

Minimal Exonum blockchain example.

## Prerequisites

To run this example you need to install [Rust](https://www.rust-lang.org/en-US/)
compiler and [third-party libraries](http://exonum.com/doc/get-started/install/).

## Build & Run

### Blockchain node

To build and run a single node use:

```sh
# clone the repository with blockchain node
git clone git@github.com:exonum/cryptocurrency.git
cd cryptocurrency

# build and run
cargo run
```

Now the node is listening HTTP requests on `localhost:8000`.

### Sample transactions

When node is launched you can use transaction examples to check it:

```sh
cd ./examples

# every `curl` call returns hash of sent transactions

# create 1st wallet and add funds
curl -H "Content-Type: application/json" -X POST -d @create-wallet-1.json \
    http://127.0.0.1:8000/api/services/cryptocurrency/v1/wallets/transaction

# create 2nd wallet and add funds
curl -H "Content-Type: application/json" -X POST -d @create-wallet-2.json \
    http://127.0.0.1:8000/api/services/cryptocurrency/v1/wallets/transaction

# transfer funds from 1st to 2nd
curl -H "Content-Type: application/json" -X POST -d @transfer-funds.json \
    http://127.0.0.1:8000/api/services/cryptocurrency/v1/wallets/transaction
```

## LICENSE

Cryptocurrency is licensed under the Apache License (Version 2.0). See [LICENSE](LICENSE) for details.
