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

//! WebSockets

mod server;

use actix::*;
use actix_web::{http, ws, HttpResponse};

use futures::IntoFuture;

use std::sync::{Arc, RwLock};
use std::time::Instant;

use api::{
    backends::actix::{self, FutureResponse, HttpRequest, RawHandler, RequestHandler},
    ServiceApiBackend, ServiceApiState,
};
use blockchain::SharedNodeState;

pub use self::server::Message;

#[derive(Clone)]
struct WsSessionState {
    addr: Addr<Syn, server::BlockCommitWs>,
}

struct WsSession {
    id: usize,
    hb: Instant,
    shared_api_state: SharedNodeState,
    server_addr: Addr<Syn, server::BlockCommitWs>,
}

impl WsSession {
    fn new(shared_api_state: SharedNodeState, server_state: WsSessionState) -> Self {
        Self {
            id: 0,
            hb: Instant::now(),
            shared_api_state,
            server_addr: server_state.addr,
        }
    }
}

impl Actor for WsSession {
    type Context = ws::WebsocketContext<Self, ServiceApiState>;

    fn started(&mut self, ctx: &mut Self::Context) {
        let addr: Addr<Syn, _> = ctx.address();
        self.server_addr
            .send(server::Subscribe {
                addr: addr.clone().recipient(),
            })
            .into_actor(self)
            .then(|res, act, ctx| {
                match res {
                    Ok(res) => {
                        act.id = res;
                    }
                    _ => ctx.stop(),
                }
                fut::ok(())
            })
            .wait(ctx);

        self.shared_api_state
            .add_subscriber(self.id, addr.recipient())
    }

    fn stopping(&mut self, _ctx: &mut <Self as Actor>::Context) -> Running {
        self.server_addr
            .do_send(server::Unsubscribe { id: self.id });
        self.shared_api_state.remove_subscriber(self.id);
        Running::Stop
    }
}

impl Handler<server::Message> for WsSession {
    type Result = ();

    fn handle(&mut self, msg: server::Message, ctx: &mut Self::Context) {
        ctx.text(msg.0);
    }
}

impl StreamHandler<ws::Message, ws::ProtocolError> for WsSession {
    fn handle(&mut self, msg: ws::Message, ctx: &mut Self::Context) {
        match msg {
            ws::Message::Ping(msg) => ctx.pong(&msg),
            ws::Message::Pong(_) => self.hb = Instant::now(),
            ws::Message::Text(_) | ws::Message::Binary(_) => {}
            ws::Message::Close(_) => {
                ctx.stop();
            }
        }
    }
}

/// WebSockets API.
#[derive(Clone, Copy, Debug)]
pub struct WebSocketsApi;

impl WebSocketsApi {
    fn handle_subscribe(
        self,
        backend: &mut actix::ApiBuilder,
        shared_api_state: SharedNodeState,
    ) -> Self {
        let server_addr = Arc::new(RwLock::new(None));

        let index = move |req: HttpRequest| -> FutureResponse {
            let server = server_addr.clone();
            let addr = server.read().unwrap();
            if addr.is_none() {
                let mut addr = server.write().unwrap();
                *addr = Some(Arbiter::start(|_| server::BlockCommitWs::default()));
            }

            let state = WsSessionState {
                addr: addr.to_owned().unwrap(),
            };

            let _ = ws::start(
                req.clone(),
                WsSession::new(shared_api_state.clone(), state.clone()),
            );

            let future = Ok(req)
                .and_then(|_req| Ok(()))
                .and_then(|value| Ok(HttpResponse::Ok().json(value)))
                .into_future();

            Box::new(future)
        };

        backend.raw_handler(RequestHandler {
            name: "ws".to_owned(),
            method: http::Method::GET,
            inner: Arc::from(index) as Arc<RawHandler>,
        });

        self
    }

    /// Adds WebSockets API endpoints to corresponding scope.
    pub fn wire(self, backend: &mut actix::ApiBuilder, shared_api_state: SharedNodeState) {
        self.handle_subscribe(backend, shared_api_state);
    }
}
