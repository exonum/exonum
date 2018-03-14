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
// spell-checker:ignore cors

#![deny(missing_debug_implementations, missing_docs)]
#![cfg_attr(feature = "flame_profile", feature(plugin, custom_attribute))]
#![cfg_attr(feature = "flame_profile", plugin(exonum_flamer))]

extern crate atty;
extern crate bit_vec;
extern crate bodyparser;
extern crate byteorder;
extern crate bytes;
extern crate chrono;
#[macro_use(crate_version, crate_authors)]
extern crate clap;
extern crate colored;
extern crate cookie;
extern crate env_logger;
#[macro_use]
extern crate exonum_profiler;
extern crate exonum_rocksdb as rocksdb;
extern crate exonum_sodiumoxide as sodiumoxide;
#[macro_use]
extern crate failure;
extern crate futures;
extern crate hex;
extern crate hyper;
extern crate iron;
extern crate iron_cors;
#[cfg(test)]
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
extern crate mount;
extern crate os_info;
extern crate params;
extern crate rand;
extern crate router;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
#[cfg(test)]
extern crate tempdir;
extern crate term;
extern crate tokio_core;
extern crate tokio_io;
extern crate tokio_retry;
#[cfg(any(test, feature = "long_benchmarks"))]
extern crate tokio_timer;
extern crate toml;
extern crate vec_map;

#[macro_use]
pub mod encoding;
#[macro_use]
pub mod messages;
#[macro_use]
pub mod helpers;
pub mod crypto;
#[doc(hidden)]
pub mod events;
pub mod node;
pub mod storage;
#[macro_use]
pub mod blockchain;
pub mod api;
