# Backend

Backend keeps balances of wallets and handles secure transactions between them.
It consists of nodes which interact with each other. Distributed nodes ensure the reliability.

## Build

To build the backend, use cargo:

```
cargo build --manifest-path=backend/Cargo.toml
```

## Run

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
