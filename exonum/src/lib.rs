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

//! Exonum blockchain framework.
//!
//! For more information see the project readme.

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
