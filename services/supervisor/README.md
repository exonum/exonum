# Exonum Supervisor Service

[![Travis Build Status](https://img.shields.io/travis/exonum/exonum/master.svg?label=Linux%20Build)](https://travis-ci.com/exonum/exonum)
[![License: Apache-2.0](https://img.shields.io/github/license/exonum/exonum.svg)](https://github.com/exonum/exonum/blob/master/LICENSE)
![rust 1.36.0+ required](https://img.shields.io/badge/rust-1.36.0+-blue.svg?label=Required%20Rust)

`exonum-supervisor` is a main service of the [Exonum blockchain framework](https://exonum.com/).
It is capable of deploying and starting new services,
stopping existing services, and changing configuration of the started services.

## Description

Supervisor is an Exonum service capable of the following activities:

- Service artifact deployment;
- Service instances creation;
- Changing consensus configuration;
- Changing service instances configuration.

More information on the artifact/service lifecycle can be found in the
documentation for the Exonum [runtime module][runtime-docs].

Supervisor service has two different operating modes: a "simple" mode and a
"decentralized" mode.

The difference between modes is in the decision making approach:

- Within the decentralized mode, to deploy a service or apply a new
  configuration, no less than (2/3)+1 validators should reach a consensus;
- Within the simple mode, any decision is executed after a single validator
  approval.

The simple mode can be useful if one network administrator manages all the
validator nodes or for testing purposes (e.g., to test service configuration
with `TestKit`).

For a network with a low node confidence, consider using the decentralized
mode.

## Interaction

The intended way to interact with supervisor is the REST API. To be precise,
requests should be sent to the one of the following endpoints:
`deploy-artifact`, `propose-config` or `confirm-config`.

Once received, supervisor will convert the request into appropriate
transaction, sign it with the validator keys and broadcast for the
rest of the network.

Key point here is that user **should not** send transactions to the supervisor
by himself. An expected format of requests for those endpoints is a serialized
Protobuf message.

To deploy an artifact, one (within the "simple" mode) or majority (within the
"decentralized" mode) of the nodes should receive a `DeployRequest` message
through API.

To request a config change, one node should receive a `ConfigPropose` message
through API.

For the "simple" mode no more actions are required. For the "decentralized"
mode the majority of the nodes should also receive `ConfigVote` messages
with a hash of the proposed configuration.

The proposal initiator that receives the original `ConfigPropose` message
must not vote for the configuration.

This node votes for the configuration propose automatically.

The operation of starting or resuming a service is treated similarly to a
configuration change and follows the same rules.

Consult [the crate docs](https://docs.rs/exonum-supervisor) for more details
about the service API.

## HTTP API

REST API of the service is documented in the crate docs.

## Usage

Include `exonum-supervisor` as a dependency in your `Cargo.toml`:

```toml
[dependencies]
exonum = "1.0.0-rc.1"
exonum-supervisor = "1.0.0-rc.1"
```

## License

`exonum-supervisor` is licensed under the Apache License (Version 2.0).
See [LICENSE](LICENSE) for details.

[runtime-docs]: https://docs.rs/exonum/latest/exonum/runtime/index.html
