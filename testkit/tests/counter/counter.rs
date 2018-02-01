// Copyright 2017 The Exonum Team
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

//! Sample counter service.

extern crate bodyparser;
extern crate iron;
extern crate router;

use exonum::blockchain::{ApiContext, Blockchain, Service, Transaction};
use exonum::messages::{Message, RawTransaction};
use exonum::node::{ApiSender, TransactionSend};
use exonum::storage::{Entry, Fork, Snapshot};
use exonum::crypto::{Hash, PublicKey};
use exonum::encoding;
use exonum::api::{Api, ApiError};
use self::iron::Handler;
use self::iron::prelude::*;
use self::router::Router;
use serde_json;

const SERVICE_ID: u16 = 1;
const TX_INCREMENT_ID: u16 = 1;

// "correct horse battery staple" brainwallet pubkey in Ed25519 with SHA-256 digest
pub const ADMIN_KEY: &str = "506f27b1b4c2403f2602d663a059b0262afd6a5bcda95a08dd96a4614a89f1b0";

// // // // Schema // // // //

pub struct CounterSchema<T> {
    view: T,
}

impl<T: AsRef<Snapshot>> CounterSchema<T> {
    pub fn new(view: T) -> Self {
        CounterSchema { view }
    }

    fn entry(&self) -> Entry<&Snapshot, u64> {
        Entry::new("counter.count", self.view.as_ref())
    }

    pub fn count(&self) -> Option<u64> {
        self.entry().get()
    }
}

impl<'a> CounterSchema<&'a mut Fork> {
    fn entry_mut(&mut self) -> Entry<&mut Fork, u64> {
        Entry::new("counter.count", self.view)
    }

    fn inc_count(&mut self, inc: u64) -> u64 {
        let count = self.count().unwrap_or(0) + inc;
        self.entry_mut().set(count);
        count
    }

    fn set_count(&mut self, count: u64) {
        self.entry_mut().set(count);
    }
}

// // // // Transactions // // // //

message! {
    struct TxIncrement {
        const TYPE = SERVICE_ID;
        const ID = TX_INCREMENT_ID;

        author: &PublicKey,
        by: u64,
    }
}

impl Transaction for TxIncrement {
    fn verify(&self) -> bool {
        self.verify_signature(self.author())
    }

    fn execute(&self, fork: &mut Fork) {
        let mut schema = CounterSchema::new(fork);
        schema.inc_count(self.by());
    }
}

message! {
    struct TxReset {
        const TYPE = SERVICE_ID;
        const ID = TX_INCREMENT_ID;

        author: &PublicKey,
    }
}

impl TxReset {
    pub fn verify_author(&self) -> bool {
        use exonum::encoding::serialize::FromHex;
        *self.author() == PublicKey::from_hex(ADMIN_KEY).unwrap()
    }
}

impl Transaction for TxReset {
    fn verify(&self) -> bool {
        self.verify_author() && self.verify_signature(self.author())
    }

    fn execute(&self, fork: &mut Fork) {
        let mut schema = CounterSchema::new(fork);
        schema.set_count(0);
    }
}

// // // // API // // // //

#[derive(Serialize, Deserialize)]
pub struct TransactionResponse {
    pub tx_hash: Hash,
}

#[derive(Clone)]
struct CounterApi {
    channel: ApiSender,
    blockchain: Blockchain,
}

impl CounterApi {
    fn increment(&self, req: &mut Request) -> IronResult<Response> {
        match req.get::<bodyparser::Struct<TxIncrement>>() {
            Ok(Some(transaction)) => {
                let transaction: Box<Transaction> = Box::new(transaction);
                let tx_hash = transaction.hash();
                self.channel.send(transaction).map_err(ApiError::from)?;
                let json = TransactionResponse { tx_hash };
                self.ok_response(&serde_json::to_value(&json).unwrap())
            }
            Ok(None) => Err(ApiError::IncorrectRequest("Empty request body".into()))?,
            Err(e) => Err(ApiError::IncorrectRequest(Box::new(e)))?,
        }
    }

    fn count(&self) -> Option<u64> {
        let view = self.blockchain.snapshot();
        let schema = CounterSchema::new(&view);
        schema.count()
    }

    fn get_count(&self, _: &mut Request) -> IronResult<Response> {
        let count = self.count().unwrap_or(0);
        self.ok_response(&serde_json::to_value(count).unwrap())
    }

    fn reset(&self, req: &mut Request) -> IronResult<Response> {
        match req.get::<bodyparser::Struct<TxReset>>() {
            Ok(Some(transaction)) => {
                let transaction: Box<Transaction> = Box::new(transaction);
                let tx_hash = transaction.hash();
                self.channel.send(transaction).map_err(ApiError::from)?;
                let json = TransactionResponse { tx_hash };
                self.ok_response(&serde_json::to_value(&json).unwrap())
            }
            Ok(None) => Err(ApiError::IncorrectRequest("Empty request body".into()))?,
            Err(e) => Err(ApiError::IncorrectRequest(Box::new(e)))?,
        }
    }

    fn wire_private(&self, router: &mut Router) {
        let self_ = self.clone();
        let reset = move |req: &mut Request| self_.reset(req);
        router.post("/reset", reset, "reset");

        // Expose `get_count` as both private and public endpoint
        // in order to test private gets as well.
        let self_ = self.clone();
        let get_count = move |req: &mut Request| self_.get_count(req);
        router.get("/count", get_count, "get_count");
    }
}

impl Api for CounterApi {
    fn wire(&self, router: &mut Router) {
        let self_ = self.clone();
        let increment = move |req: &mut Request| self_.increment(req);
        router.post("/count", increment, "increment");

        let self_ = self.clone();
        let get_count = move |req: &mut Request| self_.get_count(req);
        router.get("/count", get_count, "get_count");
    }
}

// // // // Service // // // //

pub struct CounterService;

impl Service for CounterService {
    fn service_name(&self) -> &'static str {
        "counter"
    }

    fn state_hash(&self, _: &Snapshot) -> Vec<Hash> {
        Vec::new()
    }

    fn service_id(&self) -> u16 {
        SERVICE_ID
    }

    /// Implement a method to deserialize transactions coming to the node.
    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, encoding::Error> {
        let trans: Box<Transaction> = match raw.message_type() {
            TX_INCREMENT_ID => Box::new(TxIncrement::from_raw(raw)?),
            _ => {
                return Err(encoding::Error::IncorrectMessageType {
                    message_type: raw.message_type(),
                });
            }
        };
        Ok(trans)
    }

    /// Create a REST `Handler` to process web requests to the node.
    fn public_api_handler(&self, ctx: &ApiContext) -> Option<Box<Handler>> {
        let mut router = Router::new();
        let api = CounterApi {
            channel: ctx.node_channel().clone(),
            blockchain: ctx.blockchain().clone(),
        };
        api.wire(&mut router);
        Some(Box::new(router))
    }

    fn private_api_handler(&self, ctx: &ApiContext) -> Option<Box<Handler>> {
        let mut router = Router::new();
        let api = CounterApi {
            channel: ctx.node_channel().clone(),
            blockchain: ctx.blockchain().clone(),
        };
        api.wire_private(&mut router);
        Some(Box::new(router))
    }
}
