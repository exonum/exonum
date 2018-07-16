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
use actix_web::ws;

use std::time::Instant;

use api::{backends::actix::HttpRequest, ServiceApiScope, ServiceApiState};
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
        api_scope: &mut ServiceApiScope,
        shared_api_state: SharedNodeState,
    ) -> Self {
        let server_addr = Arbiter::start(|_| server::BlockCommitWs::default());

        let state = WsSessionState { addr: server_addr };

        api_scope.resource("ws", move |_state: &ServiceApiState, req: HttpRequest| {
            let _ = ws::start(req, WsSession::new(shared_api_state.clone(), state.clone()));
            Ok(())
        });

        self
    }

    /// Adds WebSockets API endpoints to corresponding scope.
    pub fn wire(
        self,
        api_scope: &mut ServiceApiScope,
        shared_api_state: SharedNodeState,
    ) -> &mut ServiceApiScope {
        self.handle_subscribe(api_scope, shared_api_state);
        api_scope
    }
}
