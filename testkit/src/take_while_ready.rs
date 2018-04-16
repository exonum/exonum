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

use std::fmt;

use futures::{Async, Poll, Stream};

/// Stream that terminates as soon as the underlying stream does not have items ready.
pub struct TakeWhileReady<S> {
    stream: S,
    exhausted: bool,
}

impl<S> fmt::Debug for TakeWhileReady<S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
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
            Async::Ready(None) |
            Async::NotReady => {
                self.exhausted = true;
                Ok(Async::Ready(None))
            }
            Async::Ready(Some(item)) => Ok(Async::Ready(Some(item))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor;
    use futures::sync::mpsc;

    #[test]
    fn test_take_while_ready() {
        use futures::Async;
        use futures::stream::{poll_fn, iter_ok};

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
        let collected: Vec<_> = stream.wait().into_iter().collect();
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
