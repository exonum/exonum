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

#![warn(
    missing_debug_implementations,
    missing_docs,
    unsafe_code,
    bare_trait_objects
)]
#![cfg_attr(feature = "long_benchmarks", feature(test))]
// TODO This lints produces a lot of code style warnings [ERC2699]
// #![cfg_attr(feature = "cargo-clippy", warn(clippy::pedantic))]
#![cfg_attr(
    feature = "cargo-clippy",
    allow(
          // Next `cast_*` lints don't give alternatives.
          clippy::cast_possible_wrap, clippy::cast_possible_truncation, clippy::cast_sign_loss,
          // `filter(..).map(..)` often looks more shorter and readable.
          clippy::filter_map,
          // Next lints produce too much noise/false positives.
          clippy::stutter, clippy::similar_names,
          // Variant name ends with the enum name. Similar behavior to similar_names.
          clippy::pub_enum_variant_names,
          // Next lints allowed due to false positive.
          clippy::doc_markdown,
          // '... may panic' lints.
          clippy::indexing_slicing,
    )
)]

#[cfg(test)]
#[macro_use]
extern crate pretty_assertions;
extern crate actix;
extern crate actix_net;
extern crate actix_web;
extern crate atty;
extern crate bit_vec;
extern crate byteorder;
extern crate bytes;
extern crate chrono;
#[macro_use(crate_version, crate_authors)]
extern crate clap;
extern crate env_logger;
extern crate erased_serde;
pub extern crate exonum_crypto as crypto;
#[macro_use]
extern crate exonum_derive;
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
extern crate rand_xorshift;
extern crate rust_decimal;
extern crate serde;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;
extern crate ctrlc;
extern crate protobuf;
extern crate snow;
extern crate tokio;
extern crate tokio_codec;
extern crate tokio_core;
extern crate tokio_dns;
extern crate tokio_executor;
extern crate tokio_io;
extern crate tokio_retry;
extern crate tokio_threadpool;
extern crate toml;
extern crate uuid;

// Test dependencies.
#[cfg(test)]
#[macro_use]
extern crate lazy_static;
#[cfg(test)]
extern crate tempdir;
#[cfg(all(test, feature = "long_benchmarks"))]
extern crate test;

pub mod proto;
#[macro_use]
pub mod messages;
#[macro_use]
pub mod helpers;
#[macro_use]
pub mod blockchain;
pub mod api;
#[doc(hidden)]
pub mod events;
pub mod explorer;
pub mod node;
pub mod storage;

#[cfg(test)]
mod sandbox;
