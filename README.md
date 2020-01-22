# Exonum

**Status:**
[![Travis Build Status](https://img.shields.io/travis/exonum/exonum/master.svg?label=Linux)](https://travis-ci.com/exonum/exonum)
[![dependency status](https://deps.rs/repo/github/exonum/exonum/status.svg)](https://deps.rs/repo/github/exonum/exonum)
[![codecov](https://codecov.io/gh/exonum/exonum/branch/master/graph/badge.svg)](https://codecov.io/gh/exonum/exonum)

**Project info:**
[![Docs.rs](https://docs.rs/exonum/badge.svg)](https://docs.rs/exonum)
[![License: Apache-2.0](https://img.shields.io/github/license/exonum/exonum.svg)](LICENSE.md)
[![LoC](https://tokei.rs/b1/github/exonum/exonum)](https://github.com/exonum/exonum)
![rust 1.36.0+ required](https://img.shields.io/badge/rust-1.36.0+-blue.svg?label=Required%20Rust)

**Community:**
[![Join the chat at https://gitter.im/exonum/exonum](https://img.shields.io/gitter/room/exonum/exonum.svg?label=Chat)](https://gitter.im/exonum/exonum)
[![Join the chat at https://t.me/exonum_blockchain](https://img.shields.io/badge/Chat-on%20telegram-brightgreen.svg)](https://t.me/exonum_blockchain)
[![Join the chat at https://gitter.im/exonum/ruExonum](https://img.shields.io/gitter/room/exonum/ruExonum.svg?label=Russian%20chat)](https://gitter.im/exonum/ruExonum)
[![Join the chat at https://t.me/ExonumRU](https://img.shields.io/badge/Russian%20chat-on%20telegram-brightgreen.svg)](https://t.me/ExonumRU)
[![Website](https://img.shields.io/website/http/exonum.com.svg?label=Website)](https://exonum.com)

[Exonum](https://exonum.com/) is an extensible open-source framework for
creating blockchain applications. Exonum can be used to create cryptographically
powered distributed ledgers in virtually any problem domain, including FinTech,
GovTech, and LegalTech. The Exonum framework is oriented towards creating
permissioned blockchains, that is, blockchains with the known set of blockchain
infrastructure providers.

If you are using Exonum in your project and want to be listed on our website &
GitHub list — write us a line to <contact@exonum.com>.

## Contents

This is the main Exonum repository containing the bulk of Rust crates
used in Exonum. Rust crates for Exonum are intended to be reasonably
small and reusable, hence there is relatively large number of them.

### Main Crates

- [Core library](exonum/README.md)
- [Node implementation](exonum-node/README.md)
- [Node CLI](cli/README.md)

### Upstream Dependencies

- [Cryptographic library](components/crypto/README.md)
- [Database backend for merkelized data structures](components/merkledb/README.md)
- [Key management](components/keys/README.md)
- [Derive macros](components/derive/README.md)
- [Protobuf helpers](components/proto/README.md)
- [Protobuf support for build scripts](components/build/README.md)
- [High-level HTTP API abstraction](components/api/README.md)

### Tools for Building Services

- [Rust runtime](runtimes/rust/README.md)
- [Testing framework](test-suite/testkit/README.md)

### Services and Node Plugins

- [Explorer service](services/explorer/README.md) and [explorer library](components/explorer/README.md)
- [Middleware service](services/middleware/README.md)
- [Supervisor service](services/supervisor/README.md)
- [Time oracle service](services/time/README.md)
- [System API plugin](components/system-api/README.md)

### Examples

- [Cryptocurrency](examples/cryptocurrency/README.md)
- [Advanced cryptocurrency](examples/cryptocurrency-advanced/README.md)
- [Timestamping](examples/timestamping/README.md)
- [Sample runtime implementation](examples/sample_runtime/README.md)

## Versioning Policy

Exonum crates follow [semantic versioning](https://semver.org/).

The `exonum` crate and its re-exported dependencies
(`exonum-crypto`, `exonum-merkledb` and `exonum-keys`) are released
at the same time; their version is considered *the* version of the Exonum framework.
On the other hand, the crates downstream of `exonum` (e.g., `exonum-node`)
or independent of it (e.g., `exonum-api`) may evolve at different speeds,
including major releases not tied to a major Exonum release.

Throughout the Exonum codebase, certain APIs are described in the API docs
as unstable or experimental. Such APIs may be removed or changed
in a semantically non-breaking release (for example, a minor release)
of the corresponding crate.
Similarly, nominally public APIs that are hidden from the docs
via `#[doc(hidden)]` are considered unstable and thus exempt from semantic
versioning limitations.

## Contributing

To contribute to Exonum, please see [CONTRIBUTING](CONTRIBUTING.md).

## See Also

Some Exonum stuff that is *not* in this repository:

- [Java language support](https://github.com/exonum/exonum-java-binding)
- [JavaScript light client](https://github.com/exonum/exonum-client)
- [Python light client](https://github.com/exonum/exonum-python-client)
- [High-level documentation](https://github.com/exonum/exonum-doc)
- [Dynamic service launcher for Exonum](https://github.com/exonum/exonum-launcher)
