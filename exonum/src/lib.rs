// Copyright 2019 The Exonum Team
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

//! Exonum blockchain framework.
//!
//! For more information see the project readme.

#![warn(
    missing_debug_implementations,
    missing_docs,
    unsafe_code,
    bare_trait_objects
)]
#![allow(clippy::use_self)]
// #![warn(clippy::pedantic)]
// #![allow(
//     // The following `cast_*` lints do not give alternatives:
//     clippy::cast_possible_wrap, clippy::cast_possible_truncation, clippy::cast_sign_loss,
//     // `filter(..).map(..)` often looks shorter and more readable.
//     clippy::filter_map,
//     // The following lints produce too much noise/false positives:
//     clippy::module_name_repetitions, clippy::similar_names,
//     // Variant name ends with the enum name. Demonstrates similar behavior to `similar_names`.
//     clippy::pub_enum_variant_names,
//     // '... may panic' lints.
//     clippy::indexing_slicing,
//     // Suggestions for improvement that look inadequate in respect of the code that uses a lot of generics.
//     clippy::default_trait_access,
// )]

#[cfg(test)]
#[macro_use]
extern crate pretty_assertions;
pub use exonum_merkledb;
#[cfg(feature = "sodiumoxide-crypto")]
extern crate exonum_sodiumoxide as sodiumoxide;
#[macro_use]
extern crate failure;
#[macro_use]
extern crate log;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;

// Test dependencies.
#[cfg(all(test, feature = "long_benchmarks"))]
extern crate test;

pub use exonum_crypto as crypto;
pub use exonum_keys as keys;
pub use exonum_merkledb as merkledb;

#[macro_use]
pub mod messages;
#[macro_use]
pub mod helpers;
#[macro_use]
pub mod blockchain;
pub mod api;
pub mod explorer;
pub mod node;
#[macro_use]
pub mod runtime;

#[macro_use]
#[doc(hidden)]
pub mod proto;
#[doc(hidden)]
pub mod events;
