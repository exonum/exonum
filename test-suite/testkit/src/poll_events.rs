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

use futures::{Async, Future, Poll, Stream};

use std::fmt;

/// Stream that terminates as soon as the underlying stream does not have items ready.
struct TakeWhileReady<S> {
    stream: S,
    exhausted: bool,
}

impl<S> fmt::Debug for TakeWhileReady<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        f.debug_struct("TakeWhileReady")
            .field("exhausted", &self.exhausted)
            .finish()
    }
}

impl<S> TakeWhileReady<S>
where
    S: Stream,
{
    pub fn new(stream: S) -> Self {
        TakeWhileReady {
            stream,
            exhausted: false,
        }
    }
}

impl<S> Stream for TakeWhileReady<S>
where
    S: Stream,
{
    type Item = S::Item;
    type Error = S::Error;

    fn poll(&mut self) -> Poll<Option<S::Item>, S::Error> {
        if self.exhausted {
            // Do not poll the underlying stream, as it may lead to unexpected behavior
            return Ok(Async::Ready(None));
        }

        match self.stream.poll()? {
            Async::Ready(None) | Async::NotReady => {
                self.exhausted = true;
                Ok(Async::Ready(None))
            }
            Async::Ready(Some(item)) => Ok(Async::Ready(Some(item))),
        }
    }
}

/// Polls ready events from a stream of events. The stream is not closed.
pub(crate) fn poll_events<S: Stream<Item = (), Error = ()>>(stream: &mut S) {
    TakeWhileReady::new(stream.by_ref())
        .for_each(|_| Ok(()))
        .wait()
        .expect("Error polling events");
}

/// Polls ready items from the stream, returning the latest one.
pub(crate) fn poll_latest<S: Stream>(stream: &mut S) -> Option<Result<S::Item, S::Error>> {
    TakeWhileReady::new(stream).wait().last()
}

/// Polls ready items from the stream. It is assumed that a stream does not error
/// (e.g., it's an `mpsc::Receiver`).
#[cfg(feature = "exonum-node")]
pub(crate) fn poll_all<S: Stream<Error = ()>>(stream: &mut S) -> Vec<S::Item> {
    let res: Result<Vec<_>, ()> = TakeWhileReady::new(stream).wait().collect();
    res.unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::{executor, sync::mpsc};

    #[test]
    fn test_take_while_ready() {
        use futures::stream::{iter_ok, poll_fn};
        use futures::Async;

        let mut waiting = false;
        let stream = iter_ok::<_, ()>(1..4).chain(poll_fn(move || {
            if waiting {
                Ok(Async::NotReady) // hang up the stream after one produced element
            } else {
                waiting = true;
                Ok(Async::Ready(Some(4)))
            }
        }));
        let stream = TakeWhileReady::new(stream);
        let collected: Vec<_> = stream.wait().collect();
        assert_eq!(collected, vec![Ok(1), Ok(2), Ok(3), Ok(4)]);
    }

    #[test]
    fn test_take_while_ready_with_executor() {
        let (mut sender, mut receiver) = mpsc::channel(16);
        {
            let folded = TakeWhileReady::new(receiver.by_ref()).fold(0, |acc, i| Ok(acc + i));
            let mut exec = executor::spawn(folded);
            sender.try_send(1).unwrap();
            sender.try_send(2).unwrap();
            let result = exec.wait_future();
            assert_eq!(result, Ok(3));
        }

        {
            let folded = TakeWhileReady::new(receiver.by_ref()).fold(0, |acc, i| Ok(acc + i));
            let mut exec = executor::spawn(folded);
            sender.try_send(3).unwrap();
            sender.try_send(4).unwrap();
            sender.try_send(5).unwrap();
            let result = exec.wait_future();
            assert_eq!(result, Ok(12));
        }
    }
}
