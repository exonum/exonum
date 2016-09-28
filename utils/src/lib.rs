#![feature(type_ascription)]
#![feature(custom_derive)]
#![feature(plugin)]
#![plugin(serde_macros)]
#![feature(question_mark)]

extern crate rand;
extern crate time;
extern crate serde;
extern crate toml;
extern crate byteorder;
#[macro_use]
extern crate log;

#[macro_use(message)]
extern crate exonum;

pub mod config;
pub mod config_file;
