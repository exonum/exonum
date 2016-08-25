#![cfg_attr(feature="clippy", feature(plugin))]
#![cfg_attr(feature="clippy", plugin(clippy))]

#![feature(associated_consts)]
#![feature(associated_type_defaults)]
#![feature(question_mark)]
#![feature(inclusive_range_syntax)]
#![feature(type_ascription)]
#![feature(slice_concat_ext)]
#![feature(btree_range, collections_bound)]

#[macro_use]
extern crate log;
extern crate time;
extern crate byteorder;
extern crate mio;
extern crate sodiumoxide;
extern crate leveldb;
extern crate db_key;
extern crate num;
extern crate rand;

#[cfg(test)]
extern crate tempdir;

#[macro_use]
pub mod messages;
pub mod events;
pub mod crypto;
pub mod node;
pub mod storage;
pub mod blockchain;
