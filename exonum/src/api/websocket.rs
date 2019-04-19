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

//! WebSocket API.

use actix::*;
use actix_web::ws;

use rand::{rngs::ThreadRng, Rng};

use std::{cell::RefCell, collections::HashMap, sync::Arc};

use crate::api::ServiceApiState;
use crate::blockchain::Schema;
use crate::crypto::Hash;

/// WebSocket message for communication between clients(`Session`) and server(`Server`).
#[derive(Message, Debug)]
pub(crate) struct Message(pub String);

#[derive(Message)]
#[rtype(usize)]
pub(crate) struct Subscribe {
    pub address: Recipient<Message>,
}

#[derive(Message)]
pub(crate) struct Unsubscribe {
    pub id: usize,
}

#[derive(Message)]
pub(crate) struct Broadcast {
    pub block_hash: Hash,
}

pub(crate) struct Server {
    pub subscribers: HashMap<usize, Recipient<Message>>,
    service_api_state: Arc<ServiceApiState>,
    rng: RefCell<ThreadRng>,
}

impl Server {
    pub fn new(service_api_state: Arc<ServiceApiState>) -> Self {
        Self {
            subscribers: HashMap::new(),
            service_api_state,
            rng: RefCell::new(rand::thread_rng()),
        }
    }
}

impl Actor for Server {
    type Context = Context<Self>;
}

impl Handler<Subscribe> for Server {
    type Result = usize;

    fn handle(&mut self, Subscribe { address }: Subscribe, _ctx: &mut Self::Context) -> usize {
        let id = self.rng.borrow_mut().gen::<usize>();
        self.subscribers.insert(id, address);

        id
    }
}

impl Handler<Unsubscribe> for Server {
    type Result = ();

    fn handle(&mut self, Unsubscribe { id }: Unsubscribe, _ctx: &mut Self::Context) {
        self.subscribers.remove(&id);
    }
}

impl Handler<Broadcast> for Server {
    type Result = ();

    fn handle(&mut self, Broadcast { block_hash }: Broadcast, _ctx: &mut Self::Context) {
        let snapshot = self.service_api_state.snapshot();
        let schema = Schema::new(&snapshot);
        let block_header = schema.blocks().get(&block_hash);
        let block_header_json = serde_json::to_value(block_header).unwrap().to_string();
        for address in self.subscribers.values() {
            let _ = address.do_send(Message(block_header_json.clone()));
        }
    }
}

pub(crate) struct Session {
    pub id: usize,
    pub server_address: Addr<Server>,
}

impl Session {
    pub fn new(server_address: Addr<Server>) -> Self {
        Self {
            id: 0,
            server_address,
        }
    }
}

impl Actor for Session {
    type Context = ws::WebsocketContext<Self, ServiceApiState>;

    fn started(&mut self, ctx: &mut Self::Context) {
        let address: Recipient<_> = ctx.address().recipient();
        self.server_address
            .send(Subscribe { address })
            .into_actor(self)
            .then(|response, actor, context| {
                match response {
                    Ok(result) => {
                        actor.id = result;
                    }
                    _ => context.stop(),
                }
                fut::ok(())
            })
            .wait(ctx);
    }

    fn stopping(&mut self, _ctx: &mut <Self as Actor>::Context) -> Running {
        self.server_address.do_send(Unsubscribe { id: self.id });
        Running::Stop
    }
}

impl Handler<Message> for Session {
    type Result = ();

    fn handle(&mut self, msg: Message, ctx: &mut Self::Context) {
        ctx.text(msg.0);
    }
}

impl StreamHandler<ws::Message, ws::ProtocolError> for Session {
    fn handle(&mut self, msg: ws::Message, ctx: &mut Self::Context) {
        match msg {
            ws::Message::Ping(msg) => ctx.pong(&msg),
            ws::Message::Close(_) => {
                ctx.stop();
            }
            _ => {}
        }
    }
}
