# minibank

Minimal blockchain example.

## Prerequisites

To run this example you need:

* Latest [Node.js](https://nodejs.org/en/) (version 6.9.1 or above)
* Latest [Rust](https://www.rust-lang.org/en-US/) (version 1.18.0 or above)

# Build & Run

To build and run a single node use:

```sh
# clone the repository with blockchain node
git clone git@github.com:exonum/minibank.git
cd minibank

# build and run
cargo run
```

Now the node is listening HTTP requests on `localhost:8000`.

To check transactions use [cc-examples](https://github.com/exonum/cc-examples/) repository:

```sh
# clone the repository with transactions
git clone git@github.com:exonum/cc-examples.git
cd cc-examples

# use `npm login` to get access to `exonum-client`

# get all dependencies
npm install

# create 1st wallet
node create-wallet.js

# create 2nd wallet
node create-wallet-2.js

# add funds
node add-funds.js

# transfer funds from 1st to 2nd
node transfer-funds.js
```
