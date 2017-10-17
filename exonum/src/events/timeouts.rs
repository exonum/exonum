// Copyright 2017 The Exonum Team
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


use futures::{Future, Sink, Stream};
use futures::sync::mpsc;

use tokio_core::reactor::Handle;
use tokio_core::reactor::Timeout;

use std::io;
use std::time::{Duration, SystemTime};


use node::NodeTimeout;

use super::error::{into_other, other_error};
use super::{TimeoutRequest, tobox};

#[derive(Debug)]
pub struct TimeoutsPart {
    pub timeout_tx: mpsc::Sender<NodeTimeout>,
    pub timeout_requests_rx: mpsc::Receiver<TimeoutRequest>,
}

impl TimeoutsPart {
    pub fn run(self, handle: Handle) -> Box<Future<Item = (), Error = io::Error>> {
        let timeout_tx = self.timeout_tx.clone();
        let fut = self.timeout_requests_rx
            .for_each(move |request| {
                let duration = request.0.duration_since(SystemTime::now()).unwrap_or_else(
                    |_| {
                        Duration::from_millis(0)
                    },
                );
                let timeout_tx = timeout_tx.clone();
                let timeout = Timeout::new(duration, &handle)
                    .expect("Unable to create timeout")
                    .and_then(move |_| {
                        timeout_tx.clone().send(request.1).map(drop).map_err(
                            into_other,
                        )
                    })
                    .map_err(|_| panic!("Can't timeout"));
                handle.spawn(timeout);
                Ok(())
            })
            .map_err(|_| other_error("Can't handle timeout request"));
        tobox(fut)
    }
}
