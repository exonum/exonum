# Exonum Architecture

This document describes the high-level architecture of Exonum. Its target
audience is developers of Exonum core. To learn how to *use* Exonum, see
[documentation].

[documentation]: https://exonum.com/doc/

This document is provided on the best effort basis — it is not guaranteed to be
up-to-date and complete. The purpose of this document is to highlight the most
important types, modules and functions in the code, to allow new contributors to
dive into Exonum quickly. It is assumed that the reader is familiar with core
concepts of Exonum, such as [services] and [transactions].

[services]: https://exonum.com/doc/architecture/services/
[transactions]: https://exonum.com/doc/architecture/transactions/

## Code organization

The repository is a single Cargo workspace of several related packages:

- `exonum`, most of the code lives here,

- `sandbox`, where integration tests live,

- `testkit`, a framework for testing Exonum services.

This document currently describes only the `exonum` crate.

## Storage

Storage module can be found at `src/storage`. It provides an abstraction over an
embedded key-value store: the `Database` trait. The keys and values are just
slices of bytes: `&[u8]`.

To transform raw bytes into native Rust data structures, the `StorageKey` and
`StorageValue` traits are used.

On top of raw storage, collections like sets, lists and maps are provided: see
various `*Index` structs. Of particular interest are `storage/proof_map_index`
and `storage/proof_list_index` modules, which provide Merkelized collections,
capable of providing compact proofs for search queries.

Note that indexes are used both by the Exonum core and by the services. The core
uses indexes to store the internal blockchain state, which is described in the
next section. The services define and use indexes to store arbitrary
service-specific data.

## Blockchain

The format of the block of the Exonum blockchain is described in
`src/blockchain/block.rs`. Blocks are stored in the `Database`. The schema at
`src/blockchain/schema.rs` describes the indexes used for blocks, transactions
and some other data used by Exonum core.

The heart of the `Blockchain` is `Blockchain::create_patch` method, which, given
a list of transactions, executes them and produces a new `Block`.

## Node

The entities responsible for bringing all the pieces together and connecting
blockchain and services to the external world are `Node` and `NodeHandler`. Note
that there are several `impl` blocks for `NodeHandler`.

The entry point is `Node::run`, and the event dispatch loop is started in
`Node::run_handler`.

The event dispatch itself happens in `src/node/base.rs` module.

Events specific to the consensus protocol messages are handled in the
`src/node/consensus.rs` module. The messages themselves are defined in
the `src/messages/protocol.rs` module.

`Node` is also responsible for starting an HTTP API server. The API is assembled
from the built-in part, specified in `src/api` and parts, provided by each
service. The entry point of API construction is `create_private_api_handler` and
`create_public_api_handler` functions.

## Configuration

The code responsible for assembling a node with a particular set of services and
configuration lives in `src/helpers/fabric`. `NodeBuilder::run` is the "`fn
main`" of Exonum.
