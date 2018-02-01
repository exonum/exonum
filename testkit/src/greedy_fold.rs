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

use futures::{Async, Poll, Stream};

use std::fmt;

/// Implementor for greedy folding in streams.
pub struct GreedyFold<S, F, A> {
    stream: S,
    fold_fn: F,
    acc: A,
    exhausted: bool,
}

impl<S, F, A> fmt::Debug for GreedyFold<S, F, A> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        f.debug_struct("GreedyFold")
            .field("exhausted", &self.exhausted)
            .finish()
    }
}

/// Stream that folds all immediately available items from the underlying stream, yielding a single
/// resulting value. If there is no currently ready values in the stream, returns the initial value
/// of the accumulator. Once the stream is exhausted, greedy folding also stops yielding values.
///
/// # Example
///
/// ```
/// # extern crate futures;
/// # extern crate exonum_testkit;
/// use futures::{stream, Stream};
/// use exonum_testkit::GreedilyFoldable;
/// # fn main() {
/// let stream = stream::iter_ok::<_, ()>(vec![1, 2, 3, 4])
///     .greedy_fold(0, |acc, item| acc + item);
/// let values: Vec<_> = stream.wait().into_iter().collect();
/// assert_eq!(values, vec![Ok(10)]);
/// # }
/// ```
pub trait GreedilyFoldable: Stream {
    /// Performs greedy folding.
    fn greedy_fold<F, A>(self, acc: A, fold_fn: F) -> GreedyFold<Self, F, A>
    where
        F: FnMut(A, Self::Item) -> A,
        A: Clone,
        Self: Sized;
}

impl<T: Stream> GreedilyFoldable for T {
    fn greedy_fold<F, A>(self, acc: A, fold_fn: F) -> GreedyFold<Self, F, A>
    where
        F: FnMut(A, Self::Item) -> A,
        A: Clone,
        Self: Sized,
    {
        GreedyFold::new(self, acc, fold_fn)
    }
}

impl<S, F, A> GreedyFold<S, F, A>
where
    S: Stream,
    F: FnMut(A, S::Item) -> A,
    A: Clone,
{
    fn new(stream: S, acc: A, fold_fn: F) -> Self {
        GreedyFold {
            stream,
            fold_fn,
            acc,
            exhausted: false,
        }
    }
}

impl<S, F, A> Stream for GreedyFold<S, F, A>
where
    S: Stream,
    F: FnMut(A, S::Item) -> A,
    A: Clone,
{
    type Item = A;
    type Error = S::Error;

    fn poll(&mut self) -> Poll<Option<A>, S::Error> {
        if self.exhausted {
            // Do not poll the underlying stream, as it may lead to unexpected behavior
            return Ok(Async::Ready(None));
        }

        let mut acc = self.acc.clone();
        let mut some_items = false;
        loop {
            match self.stream.poll()? {
                Async::Ready(None) => {
                    self.exhausted = true;
                    return Ok(if some_items {
                        Async::Ready(Some(acc))
                    } else {
                        Async::Ready(None)
                    });
                }
                Async::NotReady => {
                    // Waiting for the next polling
                    return Ok(Async::Ready(Some(acc)));
                }
                Async::Ready(Some(item)) => {
                    some_items = true;
                    acc = (self.fold_fn)(acc, item);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor;
    use futures::sync::mpsc;

    #[test]
    fn test_fold_empty_stream() {
        //! Check that the stream continues to return the initial value of the accumulator
        let (mut sender, receiver) = mpsc::channel(1_024);
        let stream = GreedyFold::new(receiver, 0, |acc, i| acc + i);
        sender.try_send(1).unwrap();
        sender.try_send(2).unwrap();
        sender.try_send(3).unwrap();
        sender.try_send(4).unwrap();

        let folded: Vec<_> = stream.take(4).wait().into_iter().collect();
        assert_eq!(folded, vec![Ok(10), Ok(0), Ok(0), Ok(0)]);
    }

    #[test]
    fn test_iterative_fold() {
        let (mut sender, receiver) = mpsc::channel(1_024);
        let stream = GreedyFold::new(receiver, 0, |acc, i| acc + i);
        let mut exec = executor::spawn(stream);

        sender.try_send(1).unwrap();
        sender.try_send(2).unwrap();
        let result = exec.wait_stream();
        assert_eq!(result, Some(Ok(3)));

        sender.try_send(3).unwrap();
        sender.try_send(4).unwrap();
        sender.try_send(5).unwrap();
        let result = exec.wait_stream();
        assert_eq!(result, Some(Ok(12)));
    }

    #[test]
    fn test_iterative_fold_side_effects() {
        use std::cell::RefCell;

        let (mut sender, receiver) = mpsc::channel(1_024);
        let values = RefCell::new(Vec::new());
        let stream = {
            let stream = GreedyFold::new(receiver, (), |_, i| { values.borrow_mut().push(i); });
            stream
        };
        let mut exec = executor::spawn(stream);

        sender.try_send(1).unwrap();
        sender.try_send(2).unwrap();
        let result = exec.wait_stream();
        assert_eq!(result, Some(Ok(())));
        assert_eq!(*values.borrow(), vec![1, 2]);

        sender.try_send(3).unwrap();
        sender.try_send(4).unwrap();
        sender.try_send(5).unwrap();
        let result = exec.wait_stream();
        assert_eq!(result, Some(Ok(())));
        assert_eq!(*values.borrow(), vec![1, 2, 3, 4, 5]);
    }
}
