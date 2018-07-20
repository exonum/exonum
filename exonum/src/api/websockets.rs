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

use actix::*;
use actix_web::ws;

use rand::{self, Rng, ThreadRng};

use std::cell::RefCell;
use std::collections::HashMap;
use std::time::Instant;

use api::ServiceApiState;
use blockchain::SharedNodeState;
use crypto::Hash;

/// WebSocket Message for communication inside websockets part.
#[derive(Message, Debug)]
pub struct Message(pub String);

#[derive(Message)]
#[rtype(usize)]
pub(crate) struct Subscribe {
    pub addr: Recipient<Syn, Message>,
}

#[derive(Message)]
pub(crate) struct Unsubscribe {
    pub id: usize,
}

#[derive(Message)]
pub(crate) struct Broadcast {
    pub block_hash: Hash,
}

pub(crate) struct BlockCommitWs {
    pub(crate) subscribers: HashMap<usize, Recipient<Syn, Message>>,
    rng: RefCell<ThreadRng>,
}

impl Default for BlockCommitWs {
    fn default() -> Self {
        Self {
            subscribers: HashMap::new(),
            rng: RefCell::new(rand::thread_rng()),
        }
    }
}

impl Actor for BlockCommitWs {
    type Context = Context<Self>;
}

impl Handler<Subscribe> for BlockCommitWs {
    type Result = usize;

    fn handle(&mut self, msg: Subscribe, _ctx: &mut Self::Context) -> usize {
        let id = self.rng.borrow_mut().gen::<usize>();
        self.subscribers.insert(id, msg.addr);

        id
    }
}

impl Handler<Unsubscribe> for BlockCommitWs {
    type Result = ();

    fn handle(&mut self, msg: Unsubscribe, _ctx: &mut Self::Context) {
        let Unsubscribe { id } = msg;
        self.subscribers.remove(&id);
    }
}

impl Handler<Broadcast> for BlockCommitWs {
    type Result = ();

    fn handle(&mut self, msg: Broadcast, _ctx: &mut Self::Context) {
        let Broadcast { block_hash } = msg;
        for addr in self.subscribers.values() {
            let _ = addr.do_send(Message(format!("Committed new block {:?}", block_hash)));
        }
    }
}

#[derive(Clone)]
pub struct WsSessionState {
    pub(crate) addr: Addr<Syn, BlockCommitWs>,
}

pub struct WsSession {
    pub id: usize,
    pub hb: Instant,
    pub shared_api_state: SharedNodeState,
    pub(crate) server_addr: Addr<Syn, BlockCommitWs>,
}

impl WsSession {
    pub fn new(shared_api_state: SharedNodeState, server_state: WsSessionState) -> Self {
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
            .send(Subscribe {
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
    }

    fn stopping(&mut self, _ctx: &mut <Self as Actor>::Context) -> Running {
        self.server_addr.do_send(Unsubscribe { id: self.id });
        Running::Stop
    }
}

impl Handler<Message> for WsSession {
    type Result = ();

    fn handle(&mut self, msg: Message, ctx: &mut Self::Context) {
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
