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

//! A channel for sending a single message between threads.

use std::sync::mpsc;

/// Create a new one-shot channel for sending single values,
/// returning the sender/receiver halves.
pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
    let (tx, rx) = mpsc::channel();
    (Sender(tx), Receiver(rx))
}

/// The receiving half of [`channel`] type. This half can only be owned by one thread.
///
/// Message sent to the channel can be retrieved using [`wait`].
///
/// [`channel`]: fn.channel.html
/// [`wait`]: struct.Receiver.html#method.wait
#[derive(Debug)]
pub struct Receiver<T>(mpsc::Receiver<T>);

impl<T> Receiver<T> {
    /// Attempts to wait for a value on this receiver, returning an error if the
    /// corresponding channel has hung up.
    ///
    /// This function will always block the current thread if there is no data
    /// available and it's possible for more data to be sent. Once a message is
    /// sent to the corresponding [`Sender`], then this
    /// receiver will wake up and return that message.
    ///
    /// If the corresponding [`Sender`] has disconnected, or it disconnects while
    /// this call is blocking, this call will wake up and return `Err` to
    /// indicate that no more messages can ever be received on this channel.
    /// However, since channels are buffered, messages sent before the disconnect
    /// will still be properly received.
    ///
    /// [`Sender`]: struct.Sender.html
    pub fn wait(self) -> Result<T, mpsc::RecvError> {
        self.0.recv()
    }
}

/// The sending-half of [`channel`] type. This half can only be
/// owned by one thread, but it can be cloned to send to other threads.
///
/// Message can be sent through this channel with [`send`].
///
/// [`channel`]: fn.channel.html
/// [`send`]: struct.Sender.html#method.send
#[derive(Debug)]
pub struct Sender<T>(mpsc::Sender<T>);

impl<T> Sender<T> {
    /// Sends a value on this channel.
    ///
    /// This method will never block the current thread.
    ///
    /// # Panics
    ///
    /// - Panics if the corresponding receiver has already been deallocated.
    pub fn send(self, value: T) {
        self.0.send(value).expect("BUG: Unable to send message")
    }
}
