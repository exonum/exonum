// Copyright 2018 The Exonum Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Procedural macros for custom derivation in Exonum applications.
//!
//! # Deriving `MessageSet`
//!
//! Define an enum with variants representing borrowed message payloads,
//! and use `derive(MessageSet)` on it:
//!
//! ```
//! extern crate exonum;
//! #[macro_use] extern crate exonum_derive;
//! use exonum::crypto::PublicKey;
//!
//! #[derive(Debug, MessageSet)]
//! #[exonum(service_id = "100")]
//! pub enum Transactions<'a> {
//!     CreateWallet {
//!         public_key: &'a PublicKey,
//!         name: &'a str,
//!     },
//!
//!     Transfer {
//!         from: &'a PublicKey,
//!         to: &'a PublicKey,
//!         amount: u64,
//!         seed: u64,
//!     },
//! }
//! # fn main() {}
//! ```
//!
//! The macro will derive the following traits for the enum:
//!
//! - `MessageSet`
//! - `Check`, `Read` and `Write`
//! - `serde::Serialize`
//! - `ExonumJsonDeserialize<RawMessage>`
//!
//! It will also create an enum `{derived_type}Ids` containing message IDs for the messages
//! in the derived type.
//!
//! # Deriving `Message`
//!
//! Declare a newtype wrapping `exonum::messages::RawMessage`, and call `derive(Message)` on it,
//! referencing the payload type:
//!
//! ```
//! extern crate exonum;
//! #[macro_use] extern crate exonum_derive;
//! use exonum::crypto::PublicKey;
//! use exonum::messages::RawMessage;
//!
//! #[derive(Debug, MessageSet)]
//! #[exonum(service_id = "100")]
//! pub enum Transactions<'a> {
//!     Create { public_key: &'a PublicKey, name: &'a str },
//!     Transfer { from: &'a PublicKey, to: &'a PublicKey, amount: u64 },
//! }
//!
//! #[derive(Clone, Message)]
//! #[exonum(payload = "Transactions")]
//! struct Messages(RawMessage);
//! # fn main() {}
//! ```
//!
//! The macro will derive the following traits for the enum:
//!
//! - `Message`
//! - `Check` and `Read`
//! - `Debug`
//! - `AsRef<RawMessage>`
//! - `FromHex<Error = encoding::Error>`
//! - `ExonumJson`
//! - `ExonumJsonDeserialize`
//! - `serde::Serialize`
//! - `serde::Deserialize`

#![recursion_limit = "256"]

extern crate failure;
#[macro_use]
extern crate failure_derive;
extern crate proc_macro2;
#[macro_use]
extern crate quote;
#[cfg_attr(test, macro_use)]
extern crate syn;
#[macro_use]
extern crate synstructure;

mod base;
mod message;
mod structure;
mod utils;

use base::base_derive;
use message::message_derive;

decl_derive!([MessageSet, attributes(exonum)] => base_derive);
decl_derive!([Message, attributes(exonum)] => message_derive);
