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

//! Glue between WebSocket server and `actix-web` HTTP server.

use actix_web::{
    http,
    web::{Payload, Query},
    Error as ActixError, FromRequest,
};
use actix_web_actors::ws;
use exonum::blockchain::Blockchain;
use exonum_api::{
    self as api,
    backends::actix::{self as actix_backend, HttpRequest, RawHandler, RequestHandler},
    ApiBackend,
};
use exonum_rust_runtime::api::ServiceApiScope;
use futures::{Future, FutureExt};

use std::sync::Arc;

use super::{Session, SharedStateRef, SubscriptionType, TransactionFilter};
use crate::api::ExplorerApi;

impl ExplorerApi {
    /// Subscribes to events.
    fn handle_ws<Q, R>(
        name: &str,
        backend: &mut actix_backend::ApiBuilder,
        blockchain: Blockchain,
        shared_state: SharedStateRef,
        extract_query: Q,
    ) where
        Q: Fn(&HttpRequest) -> R + 'static + Clone + Send + Sync,
        R: Future<Output = Result<SubscriptionType, ActixError>> + 'static,
    {
        let index = move |request: HttpRequest, stream: Payload| {
            {
                let maybe_address = shared_state.ensure_server(&blockchain);
                let extract_query = extract_query(&request);

                async move {
                    let address = maybe_address.ok_or_else(|| {
                        let msg = "Server shut down".to_owned();
                        api::Error::not_found().title(msg)
                    })?;
                    let query = extract_query.await?;
                    ws::start(Session::new(address, vec![query]), &request, stream)
                }
            }
            .boxed_local()
        };

        backend.raw_handler(RequestHandler {
            name: name.to_owned(),
            method: http::Method::GET,
            inner: Arc::from(index) as Arc<RawHandler>,
        });
    }

    pub fn wire_ws(&self, shared_state: SharedStateRef, api_scope: &mut ServiceApiScope) -> &Self {
        // Default subscription for blocks.
        Self::handle_ws(
            "v1/blocks/subscribe",
            api_scope.web_backend(),
            self.blockchain.clone(),
            shared_state.clone(),
            |_| async { Ok(SubscriptionType::Blocks) },
        );
        // Default subscription for transactions.
        Self::handle_ws(
            "v1/transactions/subscribe",
            api_scope.web_backend(),
            self.blockchain.clone(),
            shared_state.clone(),
            |request| {
                let is_empty_query = request.query_string().is_empty();
                let extract = Query::extract(request);

                async move {
                    if is_empty_query {
                        return Ok(SubscriptionType::Transactions { filter: None });
                    }

                    extract
                        .await
                        .map(|query: Query<TransactionFilter>| {
                            Ok(SubscriptionType::Transactions {
                                filter: Some(query.into_inner()),
                            })
                        })
                        .unwrap_or(Ok(SubscriptionType::None))
                }
            },
        );
        // Default websocket connection.
        Self::handle_ws(
            "v1/ws",
            api_scope.web_backend(),
            self.blockchain.clone(),
            shared_state,
            |_| async { Ok(SubscriptionType::None) },
        );
        self
    }
}
