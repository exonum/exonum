// Copyright 2018 The Exonum Team
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

#![deny(missing_debug_implementations, missing_docs, unsafe_code, bare_trait_objects)]
#![cfg_attr(feature = "long_benchmarks", feature(test))]
#![cfg_attr(feature = "cargo-clippy", deny(clippy_pedantic))]
#![cfg_attr(
    feature = "cargo-clippy",
    allow(
          // Next `cast_*` lints don't give alternatives.
          cast_possible_wrap, cast_possible_truncation, cast_sign_loss,
          // `filter(..).map(..)` often looks more shorter and readable.
          filter_map,
          // Next lints produce too much noise/false positives.
          stutter, similar_names,
          // Variant name ends with the enum name. Similar behavior to similar_names.
          pub_enum_variant_names,
          // Next lints allowed due to false positive.
          doc_markdown,
          // Can be enabled when rust-lang-nursery/rust-clippy#2894 is fixed.
          use_self,
    )
)]

extern crate actix;
extern crate actix_web;
extern crate atty;
extern crate bit_vec;
extern crate byteorder;
extern crate bytes;
extern crate chrono;
#[macro_use(crate_version, crate_authors)]
extern crate clap;
extern crate colored;
extern crate env_logger;
extern crate exonum_rocksdb as rocksdb;
#[cfg(feature = "sodiumoxide-crypto")]
extern crate exonum_sodiumoxide as sodiumoxide;
#[macro_use]
extern crate failure;
extern crate futures;
extern crate hex;
#[macro_use]
extern crate log;
extern crate os_info;
extern crate rand;
extern crate rust_decimal;
extern crate serde;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;
extern crate snow;
extern crate term;
extern crate tokio_core;
extern crate tokio_io;
extern crate tokio_retry;
#[cfg(any(test, feature = "long_benchmarks"))]
extern crate tokio_timer;
extern crate toml;
extern crate uuid;
extern crate vec_map;

// Test dependencies.
#[cfg(test)]
#[macro_use]
extern crate lazy_static;
#[cfg(test)]
extern crate tempdir;
#[cfg(all(test, feature = "long_benchmarks"))]
extern crate test;

#[macro_use]
pub mod encoding;
#[macro_use]
pub mod messages;
#[macro_use]
pub mod helpers;
pub mod crypto;
pub mod node;
pub mod storage;
#[macro_use]
pub mod blockchain;
pub mod api;
pub mod explorer;

#[doc(hidden)]
pub mod events;

#[cfg(test)]
mod sandbox;
