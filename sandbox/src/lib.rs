#![feature(type_ascription)]

extern crate rand;
extern crate time;
extern crate exonum;
extern crate timestamping;

extern crate clap;

#[cfg(test)]
mod sandbox;
#[cfg(test)]
mod tests;

mod tx_generator;

pub use tx_generator::{TimestampingTxGenerator};
