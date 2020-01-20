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

use actix_web::{http, ws, AsyncResponder, Error as ActixError, FromRequest, HttpResponse, Query};
use exonum::blockchain::Blockchain;
use exonum_api::{
    self as api,
    backends::actix::{self as actix_backend, HttpRequest, RawHandler, RequestHandler},
    ApiBackend,
};
use exonum_rust_runtime::api::ServiceApiScope;
use futures::IntoFuture;

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
        Q: Fn(&HttpRequest) -> Result<SubscriptionType, ActixError> + Send + Sync + 'static,
    {
        let index = move |request: HttpRequest| -> Result<HttpResponse, ActixError> {
            let address = shared_state.ensure_server(&blockchain).ok_or_else(|| {
                let msg = "Server shut down".to_owned();
                api::Error::not_found().title(msg)
            })?;
            let query = extract_query(&request)?;
            ws::start(&request, Session::new(address, vec![query]))
        };
        let index = move |req| index(req).into_future().responder();

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
            |_| Ok(SubscriptionType::Blocks),
        );
        // Default subscription for transactions.
        Self::handle_ws(
            "v1/transactions/subscribe",
            api_scope.web_backend(),
            self.blockchain.clone(),
            shared_state.clone(),
            |request| {
                if request.query().is_empty() {
                    return Ok(SubscriptionType::Transactions { filter: None });
                }

                Query::from_request(request, &Default::default())
                    .map(|query: Query<TransactionFilter>| {
                        Ok(SubscriptionType::Transactions {
                            filter: Some(query.into_inner()),
                        })
                    })
                    .unwrap_or(Ok(SubscriptionType::None))
            },
        );
        // Default websocket connection.
        Self::handle_ws(
            "v1/ws",
            api_scope.web_backend(),
            self.blockchain.clone(),
            shared_state,
            |_| Ok(SubscriptionType::None),
        );
        self
    }
}
