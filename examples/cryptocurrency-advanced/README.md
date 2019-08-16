# Cryptocurrency Advanced: Service with Data Proofs

The extended version of the
[Cryptocurrency Service](https://github.com/exonum/exonum/tree/master/examples/cryptocurrency)
implementing data proofs. This project demonstrates how to bootstrap your own
cryptocurrency with [Exonum blockchain](https://github.com/exonum/exonum).

See [the documentation](https://exonum.com/doc/version/latest/get-started/data-proofs/)
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
git clone https://github.com/exonum/exonum

cd exonum/examples/cryptocurrency-advanced/backend

cargo install --path .
```

Generate template:

<!-- markdownlint-disable MD013 -->

```sh
mkdir example

exonum-cryptocurrency-advanced generate-template example/common.toml --validators-count 4
```

Generate public and secrets keys for each node:

```sh
exonum-cryptocurrency-advanced generate-config example/common.toml  example/1 --peer-address 127.0.0.1:6331 -n

exonum-cryptocurrency-advanced generate-config example/common.toml  example/2 --peer-address 127.0.0.1:6332 -n

exonum-cryptocurrency-advanced generate-config example/common.toml  example/3 --peer-address 127.0.0.1:6333 -n

exonum-cryptocurrency-advanced generate-config example/common.toml  example/4 --peer-address 127.0.0.1:6334 -n
```

Note that in case of copying files with consensus and service keys to the other machines, you must change the access permissions of these files for every machine.
For example:

```sh
sudo chmod 600 consensus.toml
sudo chmod 600 service.toml
```

Finalize configs:

```sh
exonum-cryptocurrency-advanced finalize --public-api-address 0.0.0.0:8200 --private-api-address 0.0.0.0:8091 example/1/sec.toml example/1/node.toml --public-configs example/{1,2,3,4}/pub.toml

exonum-cryptocurrency-advanced finalize --public-api-address 0.0.0.0:8201 --private-api-address 0.0.0.0:8092 example/2/sec.toml example/2/node.toml --public-configs example/{1,2,3,4}/pub.toml

exonum-cryptocurrency-advanced finalize --public-api-address 0.0.0.0:8202 --private-api-address 0.0.0.0:8093 example/3/sec.toml example/3/node.toml --public-configs example/{1,2,3,4}/pub.toml

exonum-cryptocurrency-advanced finalize --public-api-address 0.0.0.0:8203 --private-api-address 0.0.0.0:8094 example/4/sec.toml example/4/node.toml --public-configs example/{1,2,3,4}/pub.toml
```

Run nodes:

```sh
exonum-cryptocurrency-advanced run --node-config example/1/node.toml --db-path example/1/db --public-api-address 0.0.0.0:8200 --consensus-key-pass pass --service-key-pass pass

exonum-cryptocurrency-advanced run --node-config example/2/node.toml --db-path example/2/db --public-api-address 0.0.0.0:8201 --consensus-key-pass pass --service-key-pass pass

exonum-cryptocurrency-advanced run --node-config example/3/node.toml --db-path example/3/db --public-api-address 0.0.0.0:8202 --consensus-key-pass pass --service-key-pass pass

exonum-cryptocurrency-advanced run --node-config example/4/node.toml --db-path example/4/db --public-api-address 0.0.0.0:8203 --consensus-key-pass pass --service-key-pass pass
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
