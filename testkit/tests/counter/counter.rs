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
    blockchain::{ExecutionError, ExecutionResult},
    crypto::Hash,
    messages::{AnyTx, Signed},
    runtime::{
        rust::{RustArtifactId, Service, ServiceDescriptor, ServiceFactory, TransactionContext},
        ArtifactInfo, ServiceInstanceId,
    },
};
use exonum_derive::{exonum_service, ProtobufConvert};
use exonum_merkledb::{Entry, IndexAccess, ObjectHash};
use futures::{Future, IntoFuture};
use log::trace;
use serde_derive::{Deserialize, Serialize};
use std::sync::Arc;

use super::proto;

pub const SERVICE_NAME: &str = "counter";
pub const SERVICE_ID: ServiceInstanceId = 2;
/// "correct horse battery staple" brainwallet pubkey in Ed25519 with SHA-256 digest
pub const ADMIN_KEY: &str = "506f27b1b4c2403f2602d663a059b0262afd6a5bcda95a08dd96a4614a89f1b0";

pub struct CounterSchema<T> {
    access: T,
}

impl<'a, T: IndexAccess> CounterSchema<T> {
    pub fn new(access: T) -> Self {
        CounterSchema { access }
    }

    fn index_name(&self, name: &str) -> String {
        [SERVICE_NAME, ".", name].concat()
    }

    fn entry(&self) -> Entry<T, u64> {
        Entry::new(self.index_name("count"), self.access.clone())
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
pub struct TxReset;

#[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert)]
#[exonum(pb = "proto::TxIncrement")]
pub struct TxIncrement {
    by: u64,
}

impl TxIncrement {
    pub fn new(by: u64) -> Self {
        Self { by }
    }
}

#[exonum_service(dispatcher = "CounterService")]
pub trait CounterServiceInterface {
    // This method purposely does not check counter overflow in order to test
    // behavior of panicking transactions.
    fn increment(&self, context: TransactionContext, arg: TxIncrement) -> ExecutionResult;

    fn reset(&self, context: TransactionContext, arg: TxReset) -> ExecutionResult;
}

impl CounterServiceInterface for CounterService {
    fn increment(&self, context: TransactionContext, arg: TxIncrement) -> ExecutionResult {
        if arg.by == 0 {
            Err(ExecutionError::with_description(
                0,
                "Adding zero does nothing!".to_string(),
            ))?;
        }

        let mut schema = CounterSchema::new(context.fork());
        schema.inc_count(arg.by);
        Ok(())
    }

    fn reset(&self, context: TransactionContext, _arg: TxReset) -> ExecutionResult {
        let mut schema = CounterSchema::new(context.fork());
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
        transaction: Signed<AnyTx>,
    ) -> api::Result<TransactionResponse> {
        trace!("received increment tx");

        let tx_hash = transaction.object_hash();
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
        transaction: Signed<AnyTx>,
    ) -> api::Result<TransactionResponse> {
        trace!("received reset tx");

        let tx_hash = transaction.object_hash();
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

        // Check processing of custom HTTP headers. We test this using simple authorization
        // with a fixed bearer token; for practical apps, the tokens might
        // be [JSON Web Tokens](https://jwt.io/).
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

#[derive(Debug)]
pub struct CounterService;

impl Service for CounterService {
    fn wire_api(&self, _descriptor: ServiceDescriptor, builder: &mut api::ServiceApiBuilder) {
        CounterApi::wire(builder)
    }
}

impl ServiceFactory for CounterService {
    fn artifact_id(&self) -> RustArtifactId {
        "counter-service/1.0.0".parse().unwrap()
    }

    fn artifact_info(&self) -> ArtifactInfo {
        ArtifactInfo {
            proto_sources: proto::PROTO_SOURCES.as_ref(),
        }
    }

    fn create_instance(&self) -> Box<dyn Service> {
        Box::new(Self)
    }
}
