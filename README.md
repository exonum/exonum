# Cryptocurrency (demo)

Minimal Exonum blockchain example.

## Prerequisites

To run this example you need:

* Latest [Node.js](https://nodejs.org/en/) (version 6.9.1 or above)
* Latest [Rust](https://www.rust-lang.org/en-US/) (version 1.18.0 or above)

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

To check transactions use [cc-examples](https://github.com/exonum/cc-examples/) repository:

```sh
# clone the repository with transactions
git clone git@github.com:exonum/cc-examples.git
cd cc-examples

# use `npm login` to get access to `exonum-client`

# get all dependencies
npm install

# create 1st wallet and add funds
node create-wallet-1.js

# create 2nd wallet and add funds
node create-wallet-2.js

# transfer funds from 1st to 2nd
node transfer-funds.js
```

## LICENSE

Cryptocurrency is licensed under the Apache License (Version 2.0). See [LICENSE](LICENSE) for details.
