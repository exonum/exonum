# Exonum Supervisor Service

[![Travis Build Status](https://img.shields.io/travis/exonum/exonum/master.svg?label=Linux%20Build)](https://travis-ci.com/exonum/exonum)
[![License: Apache-2.0](https://img.shields.io/github/license/exonum/exonum.svg)](https://github.com/exonum/exonum/blob/master/LICENSE)
![rust 1.41.0+ required](https://img.shields.io/badge/rust-1.41.0+-blue.svg?label=Required%20Rust)

`exonum-supervisor` is a main service of the [Exonum blockchain framework](https://exonum.com/).
It is capable of deploying and starting new services,
changing the state of existing services (such as stopping, freezing,
or initiating a data migration), and changing configuration
of the started services.

## Description

Supervisor is an Exonum service capable of the following activities:

- Deploying service artifacts and unloading unused artifacts
- Instantiating services
- Changing configuration of instantiated services
- Changing a state of instantiated services: stopping, freezing, resuming,
  and initiating data migrations
- Changing consensus configuration

More information on the artifact / service lifecycle can be found in the
documentation of [service lifecycle][docs:lifecycle] and the [supervisor][docs:supervisor].

Supervisor service has two different operating modes: a "simple" mode and a
"decentralized" mode. The difference between modes is in the decision making approach:

- In the decentralized mode, to instantiate a service or apply a new
  configuration, more than 2/3rds of validators should reach a consensus.
- In the simple mode, any decision is executed after a single validator
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
"decentralized" mode) of the nodes should submit a `DeployRequest` message
through API.

To request a config change, one node should submit a `ConfigPropose` message
through API.
For the "simple" mode, no more actions are required. For the "decentralized"
mode, the majority of the nodes should also submit `ConfigVote` messages
with a hash of the proposed configuration.
The proposal initiator that sends the original `ConfigPropose` message
should not vote for the configuration; the supervisor considers that the initiator
has voted by submitting a proposal.

Starting, resuming or freezing a service, or unloading an artifact
are treated similarly to a configuration change and follow the same rules.

## HTTP API

REST API of the service is documented in [the crate docs](https://docs.rs/exonum-supervisor).

## Usage

Include `exonum-supervisor` as a dependency in your `Cargo.toml`:

```toml
[dependencies]
exonum = "1.0.0-rc.3"
exonum-supervisor = "1.0.0-rc.3"
```

Note that the supervisor service is added to the blockchain automatically
by the [`exonum-cli`] crate, so no actions are required if you use this crate.

## License

`exonum-supervisor` is licensed under the Apache License (Version 2.0).
See [LICENSE](LICENSE) for details.

[docs:supervisor]: https://exonum.com/doc/version/latest/advanced/supervisor/
[docs:lifecycle]: https://exonum.com/doc/version/latest/architecture/service-lifecycle/
[`exonum-cli`]: https://crates.io/crates/exonum-cli
