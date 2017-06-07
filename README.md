# Cryptocurrency demo

This project demonstrates how to bootstrap own cryptocurrency
with [Exonum](http://exonum.com/) blockchain.

It implements basic operations:
- create a new wallet
- add funds into a wallet
- transfer funds from the one wallet to another
- monitor blocks status

## Run this demo

Because of blockchain is a distributed kind of software you should to run
multiple nodes which handle the transactions and keep the data safely.

We prepared a minimal configuration that helps you start and test cryptocurrency right now.
Be sure you installed necessary packages:
* git
* supervisord
* node (with npm)
* bower
* Rust compiler

Than clone this project to a local folder, bootstrap and start it:

```sh
git clone https://github.com/exonum/cryptocurrency
cd cryptocurrency
SERVICE_ROOT=$(pwd)/currency_root
./service/bootstrap.sh install
./service/bootstrap.sh enable
./service/bootstrap.sh start cryptocurrency
```

Ready! Open the [wallet manager](http://127.0.0.1:3000) in your browser.

## More documentation

Follow the links if you want to read more about the [backend](doc/backend.md)
or [frontend](doc/frontend.md).
