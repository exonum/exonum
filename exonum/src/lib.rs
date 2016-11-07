#![cfg_attr(feature="clippy", feature(plugin))]
#![cfg_attr(feature="clippy", plugin(clippy))]
#![cfg_attr(test, feature(test))]

#![cfg_attr(feature="clippy", allow(zero_prefixed_literal))]

#![feature(inclusive_range_syntax)]
#![feature(type_ascription)]
#![feature(slice_concat_ext)]
#![feature(btree_range, collections_bound)]
#![feature(proc_macro)]

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
extern crate toml;
extern crate hex;
extern crate bit_vec;

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
