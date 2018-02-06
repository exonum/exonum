# Cryptocurrency demo

This project demonstrates how to bootstrap own cryptocurrency with [Exonum](http://exonum.com/) blockchain.
Exonum blockchain keeps balances of wallets and handles secure transactions between them.

It implements most basic operations:
- create a new wallet
- add funds into a wallet
- transfer funds from the one wallet to another
- monitor blocks state

### Requirements

We prepared a minimal configuration that helps you start and test cryptocurrency right now.
Be sure you installed necessary packages:
* [git](https://git-scm.com/downloads)
* [supervisord](http://supervisord.org/installing.html)
* [node](https://nodejs.org/en/download/) *(with npm)*
* [Rust compiler](https://rustup.rs/)
* [gnu-sed](https://stackoverflow.com/questions/30003570/how-to-use-gnu-sed-on-mac-os-x) on Mac OS X
* [build-essential](https://askubuntu.com/questions/398489/how-to-install-build-essential) on Ubuntu

### Quick installation on local machine

Clone the project to a local folder, bootstrap and start it:

```sh
git clone https://github.com/exonum/cryptocurrency-advanced
cd cryptocurrency-advanced
export SERVICE_ROOT=$(pwd)/currency_root
./service/bootstrap.sh install
./service/bootstrap.sh enable
./service/bootstrap.sh start cryptocurrency
```

Ready! Open the wallet manager at [http://127.0.0.1:8280](http://127.0.0.1:8280) in your browser.

To stop the project execute:
```sh
./service/bootstrap.sh stop cryptocurrency
./service/bootstrap.sh disable
./service/bootstrap.sh clear
```

### Installation with distributed nodes

Clone the project to a local folder and build it:

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
cryptocurrency run --node-config node_1_cfg.toml --rocksdb /path/to/db1 --public-api-address 0.0.0.0:8200
cryptocurrency run --node-config node_2_cfg.toml --rocksdb /path/to/db2 --public-api-address 0.0.0.0:8201
cryptocurrency run --node-config node_3_cfg.toml --rocksdb /path/to/db3 --public-api-address 0.0.0.0:8202
cryptocurrency run --node-config node_4_cfg.toml --rocksdb /path/to/db4 --public-api-address 0.0.0.0:8203
```

Next step is to install frontend application.
Start with install of frontend dependencies:

```sh
cd ../frontend
npm install
```

Clone configuration file [config-example.json](frontend/config-example.json) as `config.json`.
Fill the list of validators with validators which can be found in `consensus_public_key` field in toml config of each file.

```json
{
  "endpoint": "http://127.0.0.1:8200",
  "network_id": 0,
  "protocol_version": 0,
  "service_id": 128,
  "validators": [
    "756f0bb877333e4059e785e38d72b716a2ae9981011563cf21e60ab16bec1fbc",
    "59e785e38d72b716a2ae9981011563cf21e60a7333e40b16bec1fbc756f0bb87",
    "e99810115756f0bb877333e4059e785e38d72b716aab16bec1fbc2a63cf21e60",
    "e4059e7fbc85e38d72b716a2756f0bb877333ae9981011563cf21e60ab16bec1"
  ]
}
```

Run the application:

```sh
npm start
```

Ready! Open the wallet manager at [http://127.0.0.1:8280](http://127.0.0.1:8280) in your browser.

### Tests

Before execute tests install `karma-cli` globally:

```sh
npm install -g karma-cli
```

To execute tests run:

```sh
karma start
```

To lint code run:

```sh
npm run lint
```

### License

Cryptocurrency demo is licensed under the Apache License (Version 2.0). See [LICENSE](LICENSE) for details.
