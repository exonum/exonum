#![feature(type_ascription)]

extern crate rand;
extern crate time;
extern crate serde;
#[macro_use]
extern crate log;

extern crate clap;

extern crate exonum;
extern crate timestamping;
extern crate cryptocurrency;

mod sandbox;
mod tx_generator;

pub use tx_generator::TimestampingTxGenerator;
pub use self::sandbox::timestamping_sandbox;
