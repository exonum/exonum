//! Exonum blockchain framework.
//!
//! For more information see the project readme.

#![deny(missing_debug_implementations)]

#![cfg_attr(all(feature = "nightly", test), feature(test))]

#![cfg_attr(feature="cargo-clippy", allow(zero_prefixed_literal))]

#![cfg_attr(feature="flame_profile",feature(plugin, custom_attribute))]
#![cfg_attr(feature="flame_profile",plugin(flamer))]

extern crate profiler;
#[macro_use]
extern crate log;
extern crate byteorder;
extern crate mio;
extern crate sodiumoxide;
extern crate leveldb;
extern crate rocksdb;
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
extern crate clap;
extern crate hyper;
extern crate iron;
extern crate router;
extern crate bodyparser;
extern crate params;
extern crate cookie;
extern crate mount;

#[macro_use]
pub mod encoding;
#[macro_use]
pub mod messages;
pub mod events;
pub mod crypto;
pub mod node;
pub mod storage;
pub mod blockchain;
pub mod config;
pub mod explorer;
pub mod helpers;
pub mod api;
