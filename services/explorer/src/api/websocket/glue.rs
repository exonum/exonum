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
    FromRequest,
};
use actix_web_actors::ws;
use exonum::blockchain::Blockchain;
use exonum_api::{
    self as api,
    backends::actix::{self as actix_backend, HttpRequest, RawHandler, RequestHandler},
    ApiBackend,
};
use exonum_rust_runtime::api::ServiceApiScope;
use futures::{future, FutureExt};

use std::sync::Arc;

use super::{Session, SharedStateRef, SubscriptionType, TransactionFilter};
use crate::api::ExplorerApi;

impl ExplorerApi {
    /// Subscribes to events.
    fn handle_ws<Q>(
        name: &str,
        backend: &mut actix_backend::ApiBuilder,
        blockchain: Blockchain,
        shared_state: SharedStateRef,
        extract_query: Q,
    ) where
        Q: Fn(&HttpRequest) -> SubscriptionType + 'static + Clone + Send + Sync,
    {
        let handler = move |request: HttpRequest, stream: Payload| {
            let maybe_address = shared_state.ensure_server(&blockchain);
            let address =
                maybe_address.ok_or_else(|| api::Error::not_found().title("Server shut down"))?;

            let query = extract_query(&request);
            ws::start(Session::new(address, vec![query]), &request, stream)
        };
        let raw_handler =
            move |request, stream| future::ready(handler(request, stream)).boxed_local();

        backend.raw_handler(RequestHandler {
            name: name.to_owned(),
            method: http::Method::GET,
            inner: Arc::from(raw_handler) as Arc<RawHandler>,
        });
    }

    pub fn wire_ws(&self, shared_state: SharedStateRef, api_scope: &mut ServiceApiScope) -> &Self {
        // Default subscription for blocks.
        Self::handle_ws(
            "v1/blocks/subscribe",
            api_scope.web_backend(),
            self.blockchain.clone(),
            shared_state.clone(),
            |_| SubscriptionType::Blocks,
        );
        // Default subscription for transactions.
        Self::handle_ws(
            "v1/transactions/subscribe",
            api_scope.web_backend(),
            self.blockchain.clone(),
            shared_state.clone(),
            |request| {
                if request.query_string().is_empty() {
                    return SubscriptionType::Transactions { filter: None };
                }

                // `future::Ready<_>` type annotation is redundant; it's here to check that
                // `now_or_never` will not fail due to changes in `actix`.
                let extract: future::Ready<_> = Query::<TransactionFilter>::extract(request);
                extract
                    .now_or_never()
                    .expect("`Ready` futures always have their output immediately available")
                    .map(|query| SubscriptionType::Transactions {
                        filter: Some(query.into_inner()),
                    })
                    .unwrap_or(SubscriptionType::None)
            },
        );
        // Default websocket connection.
        Self::handle_ws(
            "v1/ws",
            api_scope.web_backend(),
            self.blockchain.clone(),
            shared_state,
            |_| SubscriptionType::None,
        );
        self
    }
}
