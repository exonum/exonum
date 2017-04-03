#![cfg_attr(feature="clippy", feature(plugin))]
#![cfg_attr(feature="clippy", plugin(clippy))]
#![cfg_attr(test, feature(test))]

#![cfg_attr(feature="clippy", allow(zero_prefixed_literal))]

#![feature(inclusive_range_syntax)]
#![feature(btree_range, collections_bound)]

#![cfg_attr(feature="flame_profile",feature(plugin, custom_attribute))]
#![cfg_attr(feature="flame_profile",plugin(flamer))]

#[macro_use]
extern crate profiler;

#[macro_use]
extern crate log;
extern crate time;
extern crate byteorder;
extern crate mio;
extern crate sodiumoxide;
extern crate leveldb;
extern crate num;
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
#[cfg(test)]
extern crate test;
#[cfg(test)]
extern crate env_logger;

#[macro_use]
pub mod messages;
pub mod events;
pub mod crypto;
pub mod node;
pub mod storage;
pub mod blockchain;
pub mod config;