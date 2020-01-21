// Copyright 2020 The Exonum Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Core library for the Exonum blockchain framework.
//!
//! Exonum is an extensible open-source framework for
//! creating blockchain applications. Exonum can be used to create cryptographically
//! powered distributed ledgers in virtually any problem domain, including finance,
//! governance, and legal. The Exonum framework is oriented towards creating
//! permissioned blockchains, that is, blockchains with the known set of blockchain
//! infrastructure providers.
//!
//! For more information about the framework see the [readme] and the [Exonum website].
//!
//! [readme]: https://github.com/exonum/exonum#readme
//! [Exonum website]: https://exonum.com/
//!
//! # Crate Overview
//!
//! This crate provides the tools necessary to work with the Exonum framework.
//!
//! ## Re-exports
//!
//! The crate re-exports the following crates:
//!
//! | Crate | Exported name | Description |
//! |-------|---------------|-------------|
//! | [`exonum-crypto`] | `crypto` | Cryptographic utils used by Exonum |
//! | [`exonum-merkledb`] | `merkledb` | Storage engine with automatic data merkelization |
//! | [`exonum-keys`] | `keys` | Key tools for Exonum nodes |
//!
//! [`exonum-crypto`]: https://docs.rs/exonum-crypto/
//! [`exonum-merkledb`]: https://docs.rs/exonum-merkledb/
//! [`exonum-keys`]: https://docs.rs/exonum-keys/
//!
//! ## Blockchain Management
//!
//! The crate provides basic tools to build Exonum nodes ([`blockchain`] and [`messages`] modules),
//! although the bulk of the node logic is placed [in a dedicated crate][`exonum-node`].
//!
//! [`blockchain`]: blockchain/index.html
//! [`messages`]: messages/index.html
//! [`exonum-node`]: https://docs.rs/exonum-node/
//!
//! ## Runtimes
//!
//! [Runtimes] are a way to attach user-provided business logic to an Exonum blockchain. This
//! logic, bundled in *services*, allows to process user transactions and interact with
//! the blockchain in other ways (e.g., via HTTP API).
//!
//! Exonum provides a [generic interface][`Runtime`] for runtimes, which allows to implement
//! services in different programming languages, for example [Rust][rust-rt] and [Java][java-rt].
//!
//! [Runtimes]: runtime/index.html
//! [`Runtime`]: runtime/trait.Runtime.html
//! [rust-rt]: https://docs.rs/exonum-rust-runtime/
//! [java-rt]: https://github.com/exonum/exonum-java-binding
//!
//! # Examples
//!
//! See the [GitHub repository][examples] for examples.
//!
//! [examples]: https://github.com/exonum/exonum/tree/master/examples

#![warn(
    missing_debug_implementations,
    missing_docs,
    unsafe_code,
    bare_trait_objects
)]
#![warn(clippy::pedantic)]
#![allow(
    // Next `cast_*` lints don't give alternatives.
    clippy::cast_possible_wrap, clippy::cast_possible_truncation, clippy::cast_sign_loss,
    // `filter(..).map(..)` often looks more shorter and readable.
    clippy::filter_map,
    // Next lints produce too much noise/false positives.
    clippy::module_name_repetitions, clippy::similar_names,
    // Variant name ends with the enum name. Similar behavior to similar_names.
    clippy::pub_enum_variant_names,
    // '... may panic' lints.
    clippy::indexing_slicing,
    clippy::use_self,
    clippy::default_trait_access,
)]

#[macro_use] // Code generated by Protobuf requires `serde_derive` macros to be globally available.
extern crate serde_derive;

pub use exonum_crypto as crypto;
pub use exonum_keys as keys;
pub use exonum_merkledb as merkledb;

#[macro_use]
pub mod messages;
pub mod blockchain;
pub mod helpers;
pub mod runtime;

#[doc(hidden)]
pub mod proto;
