// Copyright 2020 The Exonum Team
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
    blockchain::IndexProof,
    crypto::PublicKey,
    runtime::{ExecutionContext, ExecutionError, InstanceId},
};
use exonum_api::{
    backends::actix::{HttpRequest, RawHandler, RequestHandler},
    ApiBackend,
};
use exonum_derive::*;
use exonum_explorer::api::TransactionResponse;
use exonum_merkledb::{
    access::{Access, FromAccess, RawAccessMut},
    ObjectHash, ProofEntry,
};
use exonum_rust_runtime::{
    api::{self, ServiceApiBuilder, ServiceApiState},
    DefaultInstance, Service,
};
use futures::{FutureExt, TryFutureExt};
use log::trace;
use serde_derive::{Deserialize, Serialize};

use std::sync::Arc;

pub const SERVICE_NAME: &str = "counter";
pub const SERVICE_ID: InstanceId = 2;

#[derive(FromAccess, RequireArtifact)]
#[require_artifact(name = "counter-service", version = "1")]
pub struct CounterSchema<T: Access> {
    pub counter: ProofEntry<T::Base, u64>,
}

impl<T: Access> CounterSchema<T> {
    fn new(access: T) -> Self {
        Self::from_root(access).unwrap()
    }
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

#[derive(Debug, ExecutionFail)]
pub enum Error {
    /// Adding zero does nothing!
    AddingZero = 0,
    /// What's the question?
    AnswerToTheUltimateQuestion = 1,
    /// Number 13 is considered unlucky by some cultures.
    BadLuck = 2,
}

#[exonum_interface(auto_ids)]
pub trait CounterServiceInterface<Ctx> {
    type Output;

    // This method purposely does not check counter overflow in order to test
    // behavior of panicking transactions.
    fn increment(&self, ctx: Ctx, by: u64) -> Self::Output;
    fn reset(&self, ctx: Ctx, _: ()) -> Self::Output;
}

impl CounterServiceInterface<ExecutionContext<'_>> for CounterService {
    type Output = Result<(), ExecutionError>;

    fn increment(&self, context: ExecutionContext<'_>, by: u64) -> Self::Output {
        if by == 0 {
            return Err(Error::AddingZero.into());
        }

        let mut schema = CounterSchema::new(context.service_data());
        schema.inc_counter(by);
        Ok(())
    }

    fn reset(&self, context: ExecutionContext<'_>, _: ()) -> Self::Output {
        let mut schema = CounterSchema::new(context.service_data());
        schema.counter.set(0);
        Ok(())
    }
}

// // // // API // // // //

#[derive(Debug, Serialize, Deserialize)]
pub struct CounterWithProof {
    counter: Option<u64>,
    proof: IndexProof,
}

impl CounterWithProof {
    /// Verifies the proof against the known set of validators. Panics on an error.
    pub fn verify(&self, validator_keys: &[PublicKey]) -> Option<u64> {
        let (index_name, index_hash) = self.proof.verify(validator_keys).unwrap_or_else(|err| {
            panic!("Proof verification failed: {}", err);
        });
        assert_eq!(
            index_name,
            format!("{}.counter", SERVICE_NAME),
            "Invalid counter index in proof"
        );
        assert_eq!(
            index_hash,
            self.counter
                .as_ref()
                .map(ObjectHash::object_hash)
                .unwrap_or_default(),
            "Invalid counter value in proof"
        );
        self.counter
    }

    /// Mauls the proof by removing precommits.
    pub fn remove_precommits(&mut self) {
        self.proof.block_proof.precommits.clear();
    }

    /// Mauls the proof by mutating the value.
    pub fn maul_value(&mut self) {
        self.counter = Some(self.counter.unwrap_or_default() + 1);
    }
}

#[derive(Debug, Clone, Copy)]
struct CounterApi;

impl CounterApi {
    async fn increment(state: ServiceApiState, value: u64) -> api::Result<TransactionResponse> {
        trace!("received increment tx");
        let tx_hash = state
            .generic_broadcaster()
            .increment((), value)
            .await
            .map_err(|e| api::Error::internal(e).title("Failed to increment counter"))?;
        Ok(TransactionResponse::new(tx_hash))
    }

    fn count(snapshot: impl Access) -> api::Result<u64> {
        let schema = CounterSchema::new(snapshot);
        Ok(schema.counter.get().unwrap_or_default())
    }

    async fn count_with_proof(state: ServiceApiState) -> api::Result<CounterWithProof> {
        let proof = state
            .data()
            .proof_for_service_index("counter")
            .ok_or_else(|| api::Error::not_found().title("Counter not initialized"))?;
        let schema = CounterSchema::new(state.service_data());
        Ok(CounterWithProof {
            counter: schema.counter.get(),
            proof,
        })
    }

    async fn reset(state: ServiceApiState) -> api::Result<TransactionResponse> {
        trace!("received reset tx");
        // The first `()` is the empty context, the second one is the `reset` arg.
        let tx_hash = state
            .generic_broadcaster()
            .reset((), ())
            .await
            .map_err(|e| api::Error::internal(e).title("Failed to reset counter"))?;
        Ok(TransactionResponse::new(tx_hash))
    }

    fn wire(builder: &mut ServiceApiBuilder) {
        builder
            .private_scope()
            .endpoint("count", |state, _query: ()| {
                let count = Self::count(state.service_data());
                async move { count }
            })
            .endpoint_mut("reset", |state, _query: ()| Self::reset(state));
        builder
            .public_scope()
            .endpoint("count", |state, _query: ()| {
                let count = Self::count(state.service_data());
                async move { count }
            })
            .endpoint_mut("count", Self::increment);
        builder
            .public_scope()
            .endpoint("count-with-proof", |state, _query: ()| {
                Self::count_with_proof(state)
            })
            .endpoint_mut("count", Self::increment);

        // Check sending incorrect transactions via `ApiSender`. Testkit should not include
        // such transactions to the transaction pool.
        let api_sender = builder.blockchain().sender().to_owned();
        let service_keys = builder.blockchain().service_keypair().to_owned();
        builder.public_scope().endpoint_mut(
            "incorrect-tx",
            move |_state: ServiceApiState, by: u64| {
                let incorrect_tx = service_keys.increment(SERVICE_ID + 1, by);
                let hash = incorrect_tx.object_hash();
                api_sender
                    .broadcast_transaction(incorrect_tx)
                    .map_ok(move |_| hash)
                    .map_err(api::Error::internal)
            },
        );

        // Check processing of custom HTTP headers. We test this using simple authorization
        // with a fixed bearer token; for practical apps, the tokens might
        // be [JSON Web Tokens](https://jwt.io/).
        let blockchain = builder.blockchain().clone();
        let handler = move |request: HttpRequest| {
            let auth_header = request
                .headers()
                .get("Authorization")
                .ok_or_else(|| api::Error::new(api::HttpStatusCode::UNAUTHORIZED))?
                .to_str()
                .map_err(|_| api::Error::bad_request().title("Malformed `Authorization`"))?;
            if auth_header != "Bearer SUPER_SECRET_111" {
                return Err(api::Error::new(api::HttpStatusCode::UNAUTHORIZED));
            }

            let snapshot = blockchain.snapshot();
            Self::count(snapshot.as_ref())
        };
        let handler: Arc<RawHandler> = Arc::new(move |request, _payload| {
            let result = handler(request).map(|v| HttpResponse::Ok().json(v));
            async move { result.map_err(From::from) }.boxed_local()
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
#[service_factory(artifact_name = "counter-service", artifact_version = "1.0.0")]
#[service_dispatcher(implements("CounterServiceInterface"))]
pub struct CounterService;

impl DefaultInstance for CounterService {
    const INSTANCE_ID: u32 = SERVICE_ID;
    const INSTANCE_NAME: &'static str = SERVICE_NAME;
}

impl Service for CounterService {
    fn before_transactions(&self, context: ExecutionContext<'_>) -> Result<(), ExecutionError> {
        let mut schema = CounterSchema::new(context.service_data());
        if schema.counter.get() == Some(13) {
            schema.counter.set(0);
            Err(Error::BadLuck.into())
        } else {
            Ok(())
        }
    }

    fn after_transactions(&self, context: ExecutionContext<'_>) -> Result<(), ExecutionError> {
        let schema = CounterSchema::new(context.service_data());
        if schema.counter.get() == Some(42) {
            Err(Error::AnswerToTheUltimateQuestion.into())
        } else {
            Ok(())
        }
    }

    fn wire_api(&self, builder: &mut ServiceApiBuilder) {
        CounterApi::wire(builder)
    }
}
