#[macro_use]
extern crate exonum;
extern crate serde;
#[macro_use]
extern crate serde_json;
extern crate chrono;
extern crate bodyparser;

extern crate iron;
extern crate params;
extern crate router;

#[cfg(test)]
#[macro_use]
extern crate log;
#[cfg(test)]
extern crate iron_test;
#[cfg(test)]
extern crate mime;
#[cfg(test)]
extern crate sandbox;

pub mod api;
mod service;
mod blockchain;

pub use service::{TimestampingService, TIMESTAMPING_SERVICE_ID};
pub use blockchain::{TimestampingSchema, TimestampTx, Content};

pub const TIMESTAMPING_TX_ID: u16 = 0;
