# Timestamping demo

This project demonstrates how to create simple timestamping service
using [Exonum blockchain](https://github.com/exonum/exonum).

## Install and run

TODO set up backend

Install frontend dependencies:

```sh
cd frontend

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
