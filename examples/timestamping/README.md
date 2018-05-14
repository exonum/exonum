# Timestamping demo

This project demonstrates how to create simple timestamping service
using [Exonum blockchain](https://github.com/exonum/exonum).

## Getting started

Be sure you installed necessary packages:

* [git](https://git-scm.com/downloads)
* [Rust](https://rustup.rs/)
* [Node.js & npm](https://nodejs.org/en/download/)

## Install and run

Below you will find a step-by-step guide to start the service
on 4 nodes on the local machine.

Clone the project and install Rust dependencies:

```sh
git clone https://github.com/exonum/exonum

cd exonum/examples/timestamping/backend

cargo install
```

Generate blockchain configuration:

```sh
mkdir example

exonum-timestamping generate-template example/common.toml --validators-count 4
```

Generate templates of nodes configurations:

<!-- markdownlint-disable MD013 -->

```sh
exonum-timestamping generate-config example/common.toml  example/pub_1.toml example/sec_1.toml --peer-address 127.0.0.1:6331

exonum-timestamping generate-config example/common.toml  example/pub_2.toml example/sec_2.toml --peer-address 127.0.0.1:6332

exonum-timestamping generate-config example/common.toml  example/pub_3.toml example/sec_3.toml --peer-address 127.0.0.1:6333

exonum-timestamping generate-config example/common.toml  example/pub_4.toml example/sec_4.toml --peer-address 127.0.0.1:6334
```

Finalize generation of nodes configurations:

```sh
exonum-timestamping finalize --public-api-address 0.0.0.0:8200 --private-api-address 0.0.0.0:8091 example/sec_1.toml example/node_1_cfg.toml --public-configs example/pub_1.toml example/pub_2.toml example/pub_3.toml example/pub_4.toml

exonum-timestamping finalize --public-api-address 0.0.0.0:8201 --private-api-address 0.0.0.0:8092 example/sec_2.toml example/node_2_cfg.toml --public-configs example/pub_1.toml example/pub_2.toml example/pub_3.toml example/pub_4.toml

exonum-timestamping finalize --public-api-address 0.0.0.0:8202 --private-api-address 0.0.0.0:8093 example/sec_3.toml example/node_3_cfg.toml --public-configs example/pub_1.toml example/pub_2.toml example/pub_3.toml example/pub_4.toml

exonum-timestamping finalize --public-api-address 0.0.0.0:8203 --private-api-address 0.0.0.0:8094 example/sec_4.toml example/node_4_cfg.toml --public-configs example/pub_1.toml example/pub_2.toml example/pub_3.toml example/pub_4.toml
```

Run nodes:

```sh
exonum-timestamping run --node-config example/node_1_cfg.toml --db-path example/db1 --public-api-address 0.0.0.0:8200

exonum-timestamping run --node-config example/node_2_cfg.toml --db-path example/db2 --public-api-address 0.0.0.0:8201

exonum-timestamping run --node-config example/node_3_cfg.toml --db-path example/db3 --public-api-address 0.0.0.0:8202

exonum-timestamping run --node-config example/node_4_cfg.toml --db-path example/db4 --public-api-address 0.0.0.0:8203
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
npm start -- --port=2268 --api-root=http://127.0.0.1:8200
```

`--port` is a port for Node.JS app.

`--api-root` is a root URL of public API address of one of nodes.

Ready! Find demo at [http://127.0.0.1:2268](http://127.0.0.1:2268).

## License

Timestamping demo is licensed under the Apache License (Version 2.0).
See [LICENSE](LICENSE) for details.
