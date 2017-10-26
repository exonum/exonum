// Copyright 2017 The Exonum Team
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

#![deny(missing_debug_implementations)]
#![deny(missing_docs)]

#![cfg_attr(all(feature = "nightly", test), feature(test))]

#![cfg_attr(feature="cargo-clippy", allow(zero_prefixed_literal))]

#![cfg_attr(feature="flame_profile", feature(plugin, custom_attribute))]
#![cfg_attr(feature="flame_profile", plugin(exonum_flamer))]

#[macro_use]
extern crate exonum_profiler;
#[macro_use]
extern crate log;
extern crate byteorder;
extern crate mio;
extern crate exonum_sodiumoxide as sodiumoxide;
extern crate exonum_rocksdb as rocksdb;

extern crate rand;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate toml;
extern crate hex;
extern crate bit_vec;
extern crate vec_map;
#[cfg(test)]
extern crate tempdir;
#[cfg(all(feature = "nightly", test))]
extern crate test;
extern crate env_logger;
extern crate colored;
extern crate term;
#[macro_use(crate_version, crate_authors)]
extern crate clap;
extern crate hyper;
extern crate iron;
extern crate router;
extern crate params;
extern crate cookie;
extern crate mount;
extern crate atty;

#[macro_use]
pub mod encoding;
#[macro_use]
pub mod messages;
#[doc(hidden)]
pub mod events;
pub mod crypto;
pub mod node;
pub mod storage;
pub mod blockchain;
pub mod explorer;
pub mod helpers;
pub mod api;
