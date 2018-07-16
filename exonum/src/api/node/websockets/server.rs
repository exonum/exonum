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

use actix::prelude::*;

use rand::{self, Rng, ThreadRng};
use std::cell::RefCell;
use std::collections::HashMap;

/// WebSocket Message for communication inside websockets part.
#[derive(Message, Debug)]
pub struct Message(pub String);

#[derive(Message)]
#[rtype(usize)]
pub struct Subscribe {
    pub addr: Recipient<Syn, Message>,
}

#[derive(Message)]
pub struct Unsubscribe {
    pub id: usize,
}

pub struct BlockCommitWs {
    pub subscribers: HashMap<usize, Recipient<Syn, Message>>,
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

    fn handle(&mut self, msg: Subscribe, _: &mut Context<Self>) -> Self::Result {
        let id = self.rng.borrow_mut().gen::<usize>();
        self.subscribers.insert(id, msg.addr);

        id
    }
}

impl Handler<Unsubscribe> for BlockCommitWs {
    type Result = ();

    fn handle(&mut self, msg: Unsubscribe, _: &mut Context<Self>) {
        let Unsubscribe { id } = msg;
        self.subscribers.remove(&id);
    }
}
