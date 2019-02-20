# Cryptocurrency Advanced: Service with Data Proofs

The extended version of the
[Cryptocurrency Service](https://github.com/exonum/exonum/tree/master/examples/cryptocurrency)
implementing data proofs. This project demonstrates how to bootstrap your own
cryptocurrency with [Exonum blockchain](https://github.com/exonum/exonum).

See [the documentation](https://exonum.com/doc/get-started/data-proofs/)
for a detailed step-by-step guide how to approach this example.

![Cryptocurrency demo](Screenshot.png)

Exonum blockchain keeps balances of users and handles secure
transactions between them.

It implements most basic operations:

- Create a new user
- Add funds to the user's balance
- Transfer funds between users

## Install and run

### Using docker

Simply run the following command to start the cryptocurrency service on 4 nodes
on the local machine:

```bash
docker run -p 8000-8008:8000-8008 exonumhub/exonum-cryptocurrency-advanced:demo
```

Ready! Find demo at [http://127.0.0.1:8008](http://127.0.0.1:8008).

Docker will automatically pull image from the repository and
run 4 nodes with public endpoints at `127.0.0.1:8000`, ..., `127.0.0.1:8003`
and private ones at `127.0.0.1:8004`, ..., `127.0.0.1:8007`.

To stop docker container, use `docker stop <container id>` command.

### Manually

#### Getting started

Be sure you installed necessary packages:

- [git](https://git-scm.com/downloads)
- [Node.js with npm](https://nodejs.org/en/download/)
- [Rust compiler](https://rustup.rs/)

#### Install and run

Below you will find a step-by-step guide to starting the cryptocurrency
service on 4 nodes on the local machine.

Build the project:

```sh
cd examples/cryptocurrency-advanced/backend

cargo install
```

Generate template:

<!-- markdownlint-disable MD013 -->

```sh
mkdir example

exonum-cryptocurrency-advanced generate-template example/common.toml --validators-count 4
```

Generate public and secrets keys for each node:

```sh
exonum-cryptocurrency-advanced generate-config example/common.toml  example/pub_1.toml example/sec_1.toml --peer-address 127.0.0.1:6331

exonum-cryptocurrency-advanced generate-config example/common.toml  example/pub_2.toml example/sec_2.toml --peer-address 127.0.0.1:6332

exonum-cryptocurrency-advanced generate-config example/common.toml  example/pub_3.toml example/sec_3.toml --peer-address 127.0.0.1:6333

exonum-cryptocurrency-advanced generate-config example/common.toml  example/pub_4.toml example/sec_4.toml --peer-address 127.0.0.1:6334
```

Finalize configs:

```sh
exonum-cryptocurrency-advanced finalize --public-api-address 0.0.0.0:8200 --private-api-address 0.0.0.0:8091 example/sec_1.toml example/node_1_cfg.toml --public-configs example/pub_1.toml example/pub_2.toml example/pub_3.toml example/pub_4.toml

exonum-cryptocurrency-advanced finalize --public-api-address 0.0.0.0:8201 --private-api-address 0.0.0.0:8092 example/sec_2.toml example/node_2_cfg.toml --public-configs example/pub_1.toml example/pub_2.toml example/pub_3.toml example/pub_4.toml

exonum-cryptocurrency-advanced finalize --public-api-address 0.0.0.0:8202 --private-api-address 0.0.0.0:8093 example/sec_3.toml example/node_3_cfg.toml --public-configs example/pub_1.toml example/pub_2.toml example/pub_3.toml example/pub_4.toml

exonum-cryptocurrency-advanced finalize --public-api-address 0.0.0.0:8203 --private-api-address 0.0.0.0:8094 example/sec_4.toml example/node_4_cfg.toml --public-configs example/pub_1.toml example/pub_2.toml example/pub_3.toml example/pub_4.toml
```

Run nodes:

```sh
exonum-cryptocurrency-advanced run --node-config example/node_1_cfg.toml --db-path example/db1 --public-api-address 0.0.0.0:8200

exonum-cryptocurrency-advanced run --node-config example/node_2_cfg.toml --db-path example/db2 --public-api-address 0.0.0.0:8201

exonum-cryptocurrency-advanced run --node-config example/node_3_cfg.toml --db-path example/db3 --public-api-address 0.0.0.0:8202

exonum-cryptocurrency-advanced run --node-config example/node_4_cfg.toml --db-path example/db4 --public-api-address 0.0.0.0:8203
```

<!-- markdownlint-enable MD013 -->

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

## Tutorials

- Read the
  [frontend tutorial](https://github.com/exonum/exonum/blob/master/examples/cryptocurrency-advanced/tutorial/frontend.md)
  to get detailed information about the interaction of the client with Exonum blockchain.

## License

Cryptocurrency demo is licensed under the Apache License (Version 2.0).
See [LICENSE](LICENSE) for details.
