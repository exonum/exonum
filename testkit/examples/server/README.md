# Testkit Server Example

This example demonstrates how to use the testkit together with client-side testing.
The JS test script is located in the **index.js** file.

## Installation

The client-side testing is implemented using [Node][node]. Correspondingly, you
need to have Node and [npm][npm] installed. Then, install dependencies for the test
script:

```shell
npm install
```

## Usage

To run the test script, use

```shell
npm run test:unix
```

(on \*nix-based systems), or

```shell
npm test
```

(on Windows). If you use `npm test`, it is your responsibility to start
the testkit server before the test; this can be accomplished with

```shell
cargo run --example server
```

[node]: https://nodejs.org/
[npm]: https://npmjs.com/
