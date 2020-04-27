# Exonum Architecture

This document describes the high-level architecture of Exonum. Its target
audience is developers of Exonum core. To learn how to *use* Exonum, see
[documentation].

[documentation]: https://exonum.com/doc/

This document is provided on the best effort basis — it is not guaranteed to be
up-to-date and complete. The purpose of this document is to highlight the most
important types, modules and functions in the code, to allow new contributors to
dive into Exonum quickly. It is assumed that the reader is familiar with core
concepts of Exonum, such as artifacts, [services] and [transactions].

[services]: https://exonum.com/doc/version/latest/architecture/services/
[transactions]: https://exonum.com/doc/version/latest/architecture/transactions/

## Code Organization

The repository is a single Cargo workspace of
[several related packages](README.md#contents).
This document currently describes a subset of these crates, where the development
is concentrated.

## Storage

Storage module can be found in the [`exonum-merkledb`] crate.
It provides an abstraction over an embedded key-value store with column families:
the `Database` trait. The keys and values can be represented as slices of bytes:
`&[u8]`.

To transform raw bytes into native Rust data structures and vice versa, the
`BinaryKey` and `BinaryValue` traits are used.

On top of raw storage, collections like sets, lists and maps are provided; see
various `*Index` structs. Of particular interest are `ProofMapIndex` and `ProofListIndex`,
which provide *Merkelized* collections. These collections are capable
of providing compact proofs for search queries. See the corresponding modules
(`indexes::{proof_list_index, proof_map_index}`) for implementation details.

Indexes are used both by the Exonum core logic and by the services. The core
uses indexes to store the internal blockchain state, which is described in the
next section. The services define and use indexes to store arbitrary
service-specific data.

[`exonum-merkledb`]: components/merkledb

## Blockchain

The fundamentals for managing the blockchain are in the [`exonum`] crate
(aka *the core*).

The `blockchain` module defines data schema used by the core (`schema` submodule)
and related stuff like blocks (`block` submodule), configurations (`config`) and
connection to the node (`api_sender`). The shared resources of the node are packaged
in the `Blockchain` struct, which can be augmented with the behavior
to create `BlockchainMut` capable of processing transactions and generating blocks.

The behavior of a node is enclosed in the `runtime` module of the crate. The main
types there are:

- `Runtime` trait defining the interface between the core and services in a certain
  environment (e.g., Rust or JVM).
- `Dispatcher` passing calls from the core to enclosed `Runtime`s

The interface between `Dispatcher` and `BlockchainMut` is relatively small (essentially,
two main operations: creating and committing blocks), but it maps to a richer
service lifecycle defined within `Dispatcher`. Of particular note is the data
migration framework (`migrations` submodule), allowing to execute `MigrationScript`s
in background and to ensure that the migration outcome is the same among all nodes
in the network.

Note that `Dispatcher` manages lifecycle events, but does not *control* them; e.g.,
it does not initiate instantiating new services. This task is performed by
the supervisor (a service with additional privileges). One supervisor implementation
can be found in the [`exonum-supervisor`] crate. The goal of this separation
is to allow controlling the blockchain via ordinary service interface (e.g., transactions);
this minimizes the need to support this control in the core and makes lifecycle
events fully transparent.

[`exonum`]: exonum
[`exonum-supervisor`]: services/supervisor

## Node

Node logic is in the [`exonum-node`] crate. It defines the consensus algorithm,
P2P networking among nodes and provides the nodes with HTTP servers.
(The groundwork for HTTP API is located in a separate crate, [`exonum-api`].)

The entities responsible for bringing all the pieces together and connecting
`BlockchainMut` with the external world are `Node` and `NodeHandler`. Note
that there are several `impl` blocks for `NodeHandler`.
The entry point is `Node::run`, and the event dispatch loop is started in
`Node::run_handler`.

The event dispatch itself happens in the `basic` module.
Events specific to the consensus protocol messages are handled in the
`consensus` module. The messages themselves are defined in
the `messages` module, with the exception of transactions and precommits.
The latter are used by the core logic and are are thus defined
in the `exonum` crate.

HTTP API is not a part of service interface recognized by the core,
but rather is a runtime-specific interface. The Rust runtime uses the `exonum-api`
to obtain endpoint handlers from its services and sends them to the node via
an `mpsc` channel. Thus, Rust services (but not other services) share the HTTP
server with the node itself.

Besides services, HTTP API can be defined in *node plugins*. See [`exonum-system-api`]
for an example of a plugin.

[`exonum-node`]: exonum-node
[`exonum-api`]: components/api
[`exonum-system-api`]: components/system-api

## Rust Runtime

[`exonum-rust-runtime`] provides ability to write services in Rust.
Since the Rust runtime doesn't support dynamic instantiation of artifacts,
the implementation is relatively simple: the artifacts are `ServiceFactory`
trait objects that instantiate boxed `Service`s. `Service`s
implement service hooks and HTTP API endpoints.

Transaction handlers are bolted onto a service via the `ServiceDispatcher` trait,
which is usually derived via a proc macro. The handlers themselves are defined
in an implementation of an *interface* (a trait of a specific form marked
with the `exonum_interface` attribute). This allows to define multiple
interfaces for a single service. `exonum_interface` generates glue code between
the interface implementation and `ServiceDispatcher` behind the scenes;
see [`exonum-derive`] for more details. Using a clever trick, any interface trait
is automatically implemented for a set of *stubs*, such as a keypair.
See the explanation in the `stubs` module for details.

[`exonum-rust-runtime`]: runtimes/rust
[`exonum-derive`]: components/derive
