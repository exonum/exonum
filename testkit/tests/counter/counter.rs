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

//! Sample counter service.

extern crate bodyparser;
extern crate iron;
extern crate router;

use self::iron::{Handler, prelude::*};
use self::router::Router;
use exonum::api::{Api, ApiError};
use exonum::blockchain::{ApiContext, Blockchain, ExecutionError, ExecutionResult, Service,
                         Transaction, TransactionSet};
use exonum::crypto::{Hash, PublicKey};
use exonum::encoding;
use exonum::messages::{Message, RawTransaction};
use exonum::node::{ApiSender, TransactionSend};
use exonum::storage::{Entry, Fork, Snapshot};
use serde_json;

pub const SERVICE_ID: u16 = 1;

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

transactions! {
    pub CounterTransactions {
        const SERVICE_ID = SERVICE_ID;

        struct TxIncrement {
            author: &PublicKey,
            by: u64,
        }

        struct TxReset {
            author: &PublicKey,
        }
    }
}

impl Transaction for TxIncrement {
    fn verify(&self) -> bool {
        self.verify_signature(self.author())
    }

    // This method purposely does not check counter overflow in order to test
    // behavior of panicking transactions.
    fn execute(&self, fork: &mut Fork) -> ExecutionResult {
        if self.by() == 0 {
            Err(ExecutionError::with_description(
                0,
                "Adding zero does nothing!".to_string(),
            ))?;
        }

        let mut schema = CounterSchema::new(fork);
        schema.inc_count(self.by());
        Ok(())
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

    fn execute(&self, fork: &mut Fork) -> ExecutionResult {
        let mut schema = CounterSchema::new(fork);
        schema.set_count(0);
        Ok(())
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
        trace!("received increment tx");
        match req.get::<bodyparser::Struct<TxIncrement>>() {
            Ok(Some(transaction)) => {
                let transaction: Box<Transaction> = Box::new(transaction);
                let tx_hash = transaction.hash();
                self.channel.send(transaction).map_err(ApiError::from)?;
                let json = TransactionResponse { tx_hash };
                self.ok_response(&serde_json::to_value(&json).unwrap())
            }
            Ok(None) => Err(ApiError::BadRequest("Empty request body".into()))?,
            Err(e) => Err(ApiError::BadRequest(e.to_string()))?,
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
            Ok(None) => Err(ApiError::BadRequest("Empty request body".into()))?,
            Err(e) => Err(ApiError::BadRequest(e.to_string()))?,
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
    fn service_name(&self) -> &str {
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
        let tx = CounterTransactions::tx_from_raw(raw)?;
        Ok(tx.into())
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
