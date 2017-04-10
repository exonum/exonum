#[macro_use(message, storage_value)]
extern crate exonum;
extern crate blockchain_explorer;
extern crate serde;
extern crate serde_json;
extern crate chrono;
#[macro_use]
extern crate derive_error;

extern crate iron;
extern crate params;
extern crate router;

pub mod api;
mod service;
mod blockchain;

pub use service::{TimestampingService, TIMESTAMPING_SERVICE_ID};
pub use blockchain::{TimestampingSchema, TimestampTx, Content};

pub const TIMESTAMPING_TX_ID: u16 = 0;
