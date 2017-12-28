#![cfg_attr(feature="cargo-clippy", allow(zero_prefixed_literal))]

#[macro_use]
extern crate exonum;
extern crate serde;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;
extern crate bodyparser;

extern crate iron;
extern crate router;
extern crate params;

#[macro_use]
extern crate log;
#[cfg(test)]
#[macro_use]
extern crate exonum_testkit;

pub mod api;
pub mod blockchain;
mod service;

pub use service::{TimestampingService, TIMESTAMPING_SERVICE};

pub const TIMESTAMPING_TX_ID: u16 = 0;
