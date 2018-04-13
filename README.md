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
git clone https://github.com/exonum/timestamping-demo

cd timestamping-demo/backend

cargo install
```

TODO set up backend

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
