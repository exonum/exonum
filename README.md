# Cryptocurrency demo

This project demonstrates how to bootstrap own cryptocurrency
with [Exonum blockchain](https://github.com/exonum/exonum).

Exonum blockchain keeps balances of users and handles secure
transactions between them.

It implements most basic operations:
- Create a new user
- Add funds to the user's balance
- Transfer funds between users

## Getting started

Be sure you installed necessary packages:

* [git](https://git-scm.com/downloads)
* [Node.js with npm](https://nodejs.org/en/download/)
* [Rust compiler](https://rustup.rs/)

## Install and run

Below you will find a step-by-step guide to starting the cryptocurrency
service on 4 nodes on the local machine.

Clone the project and build it:

```sh
git clone https://github.com/exonum/cryptocurrency-advanced

cd cryptocurrency-advanced/backend

cargo install
```

Generate template:

```sh
cd .. && mkdir example && cd example

cryptocurrency generate-template common.toml
```

Generate public and secrets keys for each node:

```sh
cryptocurrency generate-config common.toml  pub_1.toml sec_1.toml --peer-addr 127.0.0.1:6331

cryptocurrency generate-config common.toml  pub_2.toml sec_2.toml --peer-addr 127.0.0.1:6332

cryptocurrency generate-config common.toml  pub_3.toml sec_3.toml --peer-addr 127.0.0.1:6333

cryptocurrency generate-config common.toml  pub_4.toml sec_4.toml --peer-addr 127.0.0.1:6334
```

Finalize configs:

```sh
cryptocurrency finalize --public-api-address 0.0.0.0:8200 --private-api-address 0.0.0.0:8091 sec_1.toml node_1_cfg.toml --public-configs pub_1.toml pub_2.toml pub_3.toml pub_4.toml

cryptocurrency finalize --public-api-address 0.0.0.0:8201 --private-api-address 0.0.0.0:8092 sec_2.toml node_2_cfg.toml --public-configs pub_1.toml pub_2.toml pub_3.toml pub_4.toml

cryptocurrency finalize --public-api-address 0.0.0.0:8202 --private-api-address 0.0.0.0:8093 sec_3.toml node_3_cfg.toml --public-configs pub_1.toml pub_2.toml pub_3.toml pub_4.toml

cryptocurrency finalize --public-api-address 0.0.0.0:8203 --private-api-address 0.0.0.0:8094 sec_4.toml node_4_cfg.toml --public-configs pub_1.toml pub_2.toml pub_3.toml pub_4.toml
```

Run nodes:

```sh
cryptocurrency run --node-config node_1_cfg.toml --db-path db1 --public-api-address 0.0.0.0:8200

cryptocurrency run --node-config node_2_cfg.toml --db-path db2 --public-api-address 0.0.0.0:8201

cryptocurrency run --node-config node_3_cfg.toml --db-path db3 --public-api-address 0.0.0.0:8202

cryptocurrency run --node-config node_4_cfg.toml --db-path db4 --public-api-address 0.0.0.0:8203
```

Install frontend dependencies:

```sh
cd ../frontend

npm install
```

Build sources:

```sh
npm run build
```

Run the application:

```sh
npm start -- --port=8280 --api-root=http://127.0.0.1:8200
```

`--port` is a port for Node.JS app.

`--api-root` is a root URL of public API address of one of nodes.

Ready! Find demo at [http://127.0.0.1:8280](http://127.0.0.1:8280).

## Blockchain monitoring

Use the official [blockchain explorer](https://github.com/exonum/blockchain-explorer)
to monitor blocks and transactions in the blockchain.

Use root URL of public API address of one of nodes as `--api-root` parameter, e.g `http://127.0.0.1:8200`.

## Tutorials

* [Frontend tutorial](tutorial/frontend.md)

## License

Cryptocurrency demo is licensed under the Apache License (Version 2.0). See [LICENSE](LICENSE) for details.
