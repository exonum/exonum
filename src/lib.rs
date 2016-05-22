#![allow(dead_code)]

#![feature(associated_consts)]
#![feature(associated_type_defaults)]
#![feature(question_mark)]

#[macro_use]
extern crate log;
extern crate env_logger;
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
pub mod state;
pub mod node;
