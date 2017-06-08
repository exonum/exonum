# Cryptocurrency demo

This project demonstrates how to bootstrap own cryptocurrency
with [Exonum](http://exonum.com/) blockchain.

It implements basic operations:
- create a new wallet
- add funds into a wallet
- transfer funds from the one wallet to another
- monitor blocks status

## Demo

Because of blockchain is a distributed kind of software you should to run
multiple nodes which handle the transactions and keep the data safely.

### Requirements

We prepared a minimal configuration that helps you start and test cryptocurrency
right now. Be sure you installed necessary packages:
* git
* supervisord
* node (with npm)
* bower
* Rust compiler

### Run

Clone this project to a local folder, bootstrap and start it:

```sh
git clone https://github.com/exonum/cryptocurrency
cd cryptocurrency
SERVICE_ROOT=$(pwd)/currency_root
./service/bootstrap.sh install
./service/bootstrap.sh enable
./service/bootstrap.sh start cryptocurrency
```

Ready! Open the [wallet manager](http://127.0.0.1:3000) in your browser.

## Backend

Backend keeps balances of wallets and handles secure transactions between them.
It consists of nodes which interact with each other. Distributed nodes ensure the reliability.

### Build

To build the backend, use cargo:

```
cargo build --manifest-path=backend/Cargo.toml
```

### Run

When backend was built, you should declare all nodes to run.
There is a special command `generate` which does it automatically:

```
cryptocurrency generate 4 --output-dir=example
```

In the example above we created configs for 4 nodes and put them into `example/` folder.

The next step you should start all nodes:

```
cryptocurrency --leveldb-path=example/0 --node-config=example/validators/0.toml --public-api-address=127.0.0.1:8000
cryptocurrency --leveldb-path=example/1 --node-config=example/validators/1.toml
cryptocurrency --leveldb-path=example/2 --node-config=example/validators/2.toml
cryptocurrency --leveldb-path=example/3 --node-config=example/validators/3.toml
```

## Frontend

Frontend is a lightweight single page application served by Node.js.
It communicates with the backend via REST API and uses [Exonum client](https://github.com/exonum/exonum-client) library to parse and verify data and perform cryptographic operations.

All business logic is can be found in [cryptocurrency.js](frontend/js/cryptocurrency.js).

### How it works?

Find detailed [step-by-step tutorial](http://exonum.com/doc/home/cryptocurrency/intro/) how to set up all this demo functionality from the very beginning.

### Build

Install npm dependencies:

```
npm install
```

Install bower dependencies:

```
bower install
```

### Run

To run application:

```
cd frontend
npm start
```

Application is served on [http://127.0.0.1:3000](http://127.0.0.1:3000).
