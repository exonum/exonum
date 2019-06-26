// Copyright 2019 The Exonum Team
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

use actix_web::{http::Method, HttpResponse};
use exonum::{
    api::backends::actix::{HttpRequest, RawHandler, RequestHandler},
    api::{self, ServiceApiBackend},
    blockchain::{
        ExecutionError, ExecutionResult, Service, Transaction, TransactionContext, TransactionSet,
    },
    crypto::{Hash, PublicKey, SecretKey},
    messages::{Message, RawTransaction, Signed},
};
use exonum_derive::*;
use exonum_merkledb::{Entry, IndexAccess, Snapshot};
use futures::{Future, IntoFuture};
use log::trace;
use serde_derive::*;

use std::{borrow::Cow, sync::Arc};

use super::proto;

pub const SERVICE_ID: u16 = 1;

// "correct horse battery staple" brainwallet pubkey in Ed25519 with SHA-256 digest
pub const ADMIN_KEY: &str = "506f27b1b4c2403f2602d663a059b0262afd6a5bcda95a08dd96a4614a89f1b0";

// // // // Schema // // // //

pub struct CounterSchema<T> {
    view: T,
}

impl<T: IndexAccess> CounterSchema<T> {
    pub fn new(view: T) -> Self {
        CounterSchema { view }
    }

    fn entry(&self) -> Entry<T, u64> {
        Entry::new("counter.count", self.view.clone())
    }

    pub fn count(&self) -> Option<u64> {
        self.entry().get()
    }

    fn inc_count(&mut self, inc: u64) -> u64 {
        let count = self
            .count()
            .unwrap_or(0)
            .checked_add(inc)
            .expect("attempt to add with overflow");
        self.entry().set(count);
        count
    }

    fn set_count(&mut self, count: u64) {
        self.entry().set(count);
    }
}

// // // // Transactions // // // //

#[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert)]
#[exonum(pb = "proto::TxReset")]
pub struct TxReset {}

#[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert)]
#[exonum(pb = "proto::TxIncrement")]
pub struct TxIncrement {
    by: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, TransactionSet)]
pub enum CounterTransactions {
    Increment(TxIncrement),
    Reset(TxReset),
}

impl TxIncrement {
    pub fn new(by: u64) -> Self {
        Self { by }
    }

    pub fn sign(author: &PublicKey, by: u64, key: &SecretKey) -> Signed<RawTransaction> {
        Message::sign_transaction(Self::new(by), SERVICE_ID, *author, key)
    }
}

impl Transaction for TxIncrement {
    // This method purposely does not check counter overflow in order to test
    // behavior of panicking transactions.
    fn execute(&self, tc: TransactionContext) -> ExecutionResult {
        if self.by == 0 {
            Err(ExecutionError::with_description(
                0,
                "Adding zero does nothing!".to_string(),
            ))?;
        }

        let mut schema = CounterSchema::new(tc.fork());
        schema.inc_count(self.by);
        Ok(())
    }
}

impl TxReset {
    pub fn sign(author: &PublicKey, key: &SecretKey) -> Signed<RawTransaction> {
        Message::sign_transaction(Self {}, SERVICE_ID, *author, key)
    }
}

impl Transaction for TxReset {
    fn execute(&self, tc: TransactionContext) -> ExecutionResult {
        let mut schema = CounterSchema::new(tc.fork());
        schema.set_count(0);
        Ok(())
    }
}

// // // // API // // // //

#[derive(Debug, Serialize, Deserialize)]
pub struct TransactionResponse {
    pub tx_hash: Hash,
}

#[derive(Debug, Clone, Copy)]
struct CounterApi;

impl CounterApi {
    fn increment(
        state: &api::ServiceApiState,
        transaction: Signed<RawTransaction>,
    ) -> api::Result<TransactionResponse> {
        trace!("received increment tx");

        let tx_hash = transaction.hash();
        state.sender().broadcast_transaction(transaction)?;
        Ok(TransactionResponse { tx_hash })
    }

    fn count(state: &api::ServiceApiState, _query: ()) -> api::Result<u64> {
        let snapshot = state.snapshot();
        let schema = CounterSchema::new(&snapshot);
        Ok(schema.count().unwrap_or_default())
    }

    fn reset(
        state: &api::ServiceApiState,
        transaction: Signed<RawTransaction>,
    ) -> api::Result<TransactionResponse> {
        trace!("received reset tx");

        let tx_hash = transaction.hash();
        state.sender().broadcast_transaction(transaction)?;
        Ok(TransactionResponse { tx_hash })
    }

    fn wire(builder: &mut api::ServiceApiBuilder) {
        builder
            .private_scope()
            .endpoint("count", Self::count)
            .endpoint_mut("reset", Self::reset);
        builder
            .public_scope()
            .endpoint("count", Self::count)
            .endpoint_mut("count", Self::increment);

        // Check processing of custom HTTP headers. We test this using strawman authorization
        // with a fixed bearer token; for practical apps, the tokens might
        // be [JWTs](https://jwt.io/).
        let handler = |request: HttpRequest| -> api::Result<u64> {
            let auth_header = request
                .headers()
                .get("Authorization")
                .ok_or_else(|| api::Error::Unauthorized)?
                .to_str()
                .map_err(|_| api::Error::BadRequest("Malformed `Authorization`".to_owned()))?;
            if auth_header != "Bearer SUPER_SECRET_111" {
                return Err(api::Error::Unauthorized);
            }

            let state = request.state();
            Self::count(&state, ())
        };
        let handler: Arc<RawHandler> = Arc::new(move |request| {
            Box::new(
                handler(request)
                    .into_future()
                    .from_err()
                    .map(|v| HttpResponse::Ok().json(v)),
            )
        });

        builder
            .public_scope()
            .web_backend()
            .raw_handler(RequestHandler {
                name: "v1/counter-with-auth".to_string(),
                method: Method::GET,
                inner: handler,
            });
    }
}

// // // // Service // // // //

pub struct CounterService;

impl Service for CounterService {
    fn service_name(&self) -> &str {
        "counter"
    }

    fn state_hash(&self, _: &dyn Snapshot) -> Vec<Hash> {
        Vec::new()
    }

    fn service_id(&self) -> u16 {
        SERVICE_ID
    }

    /// Implement a method to deserialize transactions coming to the node.
    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<dyn Transaction>, failure::Error> {
        let tx = CounterTransactions::tx_from_raw(raw)?;
        Ok(tx.into())
    }

    fn wire_api(&self, builder: &mut api::ServiceApiBuilder) {
        CounterApi::wire(builder)
    }
}
