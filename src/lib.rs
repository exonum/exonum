#![allow(dead_code)]

#![feature(associated_consts)]
#![feature(associated_type_defaults)]
#![feature(question_mark)]
#![feature(inclusive_range_syntax)]
#![feature(type_ascription)]
#![feature(slice_concat_ext)]
#![feature(zero_one)]

#[macro_use]
extern crate log;
extern crate env_logger;
extern crate rand;
extern crate time;
extern crate byteorder;
extern crate mio;
extern crate sodiumoxide;

#[macro_use]
pub mod messages;
pub mod connection;
pub mod network;
pub mod events;
pub mod crypto;
pub mod node;
pub mod storage;
pub mod storage2;

pub mod tx_generator;
