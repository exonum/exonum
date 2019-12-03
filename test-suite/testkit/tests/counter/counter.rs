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
    api::{
        self,
        backends::actix::{HttpRequest, RawHandler, RequestHandler},
        ApiBackend,
    },
    crypto::Hash,
    runtime::{
        rust::{
            api::{ServiceApiBuilder, ServiceApiState},
            CallContext, Service,
        },
        BlockchainData, ExecutionError, InstanceId,
    },
};
use exonum_derive::*;
use exonum_merkledb::{
    access::{Access, RawAccessMut},
    Entry, Snapshot,
};
use exonum_proto::ProtobufConvert;
use futures::{Future, IntoFuture};
use log::trace;
use serde_derive::{Deserialize, Serialize};

use std::sync::Arc;

use super::proto;

pub const SERVICE_NAME: &str = "counter";
pub const SERVICE_ID: InstanceId = 2;
/// "correct horse battery staple" brainwallet pubkey in Ed25519 with a SHA-256 digest
pub const ADMIN_KEY: &str = "506f27b1b4c2403f2602d663a059b0262afd6a5bcda95a08dd96a4614a89f1b0";

#[derive(FromAccess)]
pub struct CounterSchema<T: Access> {
    pub counter: Entry<T::Base, u64>,
}

impl<T> CounterSchema<T>
where
    T: Access,
    T::Base: RawAccessMut,
{
    fn inc_counter(&mut self, inc: u64) -> u64 {
        let count = self
            .counter
            .get()
            .unwrap_or(0)
            .checked_add(inc)
            .expect("attempt to add with overflow");
        self.counter.set(count);
        count
    }
}

// // // // Transactions // // // //

#[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "proto::TxReset")]
pub struct Reset;

#[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "proto::TxIncrement")]
pub struct Increment {
    by: u64,
}

impl Increment {
    pub fn new(by: u64) -> Self {
        Self { by }
    }
}

#[derive(Debug, ExecutionFail)]
pub enum Error {
    /// Adding zero does nothing!
    AddingZero = 0,
}

#[exonum_interface]
pub trait CounterServiceInterface {
    // This method purposely does not check counter overflow in order to test
    // behavior of panicking transactions.
    fn increment(&self, context: CallContext<'_>, arg: Increment) -> Result<(), ExecutionError>;

    fn reset(&self, context: CallContext<'_>, arg: Reset) -> Result<(), ExecutionError>;
}

impl CounterServiceInterface for CounterService {
    fn increment(&self, context: CallContext<'_>, arg: Increment) -> Result<(), ExecutionError> {
        if arg.by == 0 {
            return Err(Error::AddingZero.into());
        }

        let mut schema = CounterSchema::new(context.service_data());
        schema.inc_counter(arg.by);
        Ok(())
    }

    fn reset(&self, context: CallContext<'_>, _arg: Reset) -> Result<(), ExecutionError> {
        let mut schema = CounterSchema::new(context.service_data());
        schema.counter.set(0);
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
    fn increment(state: &ServiceApiState<'_>, value: u64) -> api::Result<TransactionResponse> {
        trace!("received increment tx");
        let tx_hash = state.generic_broadcaster().send(Increment::new(value))?;
        Ok(TransactionResponse { tx_hash })
    }

    fn count(snapshot: impl Access) -> api::Result<u64> {
        let schema = CounterSchema::new(snapshot);
        Ok(schema.counter.get().unwrap_or_default())
    }

    fn reset(state: &ServiceApiState<'_>) -> api::Result<TransactionResponse> {
        trace!("received reset tx");
        let tx_hash = state.generic_broadcaster().send(Reset)?;
        Ok(TransactionResponse { tx_hash })
    }

    fn wire(builder: &mut ServiceApiBuilder) {
        builder
            .private_scope()
            .endpoint("count", |state, _query: ()| {
                Self::count(state.service_data())
            })
            .endpoint_mut("reset", |state, _query: ()| Self::reset(state));
        builder
            .public_scope()
            .endpoint("count", |state, _query: ()| {
                Self::count(state.service_data())
            })
            .endpoint_mut("count", Self::increment);

        // Check processing of custom HTTP headers. We test this using simple authorization
        // with a fixed bearer token; for practical apps, the tokens might
        // be [JSON Web Tokens](https://jwt.io/).
        let blockchain = builder.blockchain().clone();
        let handler = move |request: HttpRequest| -> api::Result<u64> {
            let auth_header = request
                .headers()
                .get("Authorization")
                .ok_or_else(|| api::Error::Unauthorized)?
                .to_str()
                .map_err(|_| api::Error::BadRequest("Malformed `Authorization`".to_owned()))?;
            if auth_header != "Bearer SUPER_SECRET_111" {
                return Err(api::Error::Unauthorized);
            }

            let snapshot = blockchain.snapshot();
            Self::count(snapshot.as_ref())
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

#[derive(Debug, ServiceDispatcher, ServiceFactory)]
#[service_factory(
    artifact_name = "counter-service",
    artifact_version = "1.0.0",
    proto_sources = "crate::proto"
)]
#[service_dispatcher(implements("CounterServiceInterface"))]
pub struct CounterService;

impl Service for CounterService {
    fn state_hash(&self, _data: BlockchainData<&dyn Snapshot>) -> Vec<Hash> {
        vec![]
    }

    fn wire_api(&self, builder: &mut ServiceApiBuilder) {
        CounterApi::wire(builder)
    }
}
