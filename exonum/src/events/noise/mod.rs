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

use byteorder::{ByteOrder, LittleEndian};
use futures::future::{done, err, Future};
use tokio_core::net::TcpStream;
use tokio_io::{codec::Framed,
               io::{read_exact, write_all},
               AsyncRead,
               AsyncWrite};

use std::io;

use crypto::{PublicKey, SecretKey};
use events::noise::wrapper::NOISE_MAX_HANDSHAKE_MESSAGE_LENGTH;
use events::{codec::MessagesCodec,
             noise::wrapper::{NoiseWrapper, HANDSHAKE_HEADER_LENGTH}};

pub mod wrapper;

#[cfg(test)]
mod tests;

type HandshakeResult = Box<Future<Item = Framed<TcpStream, MessagesCodec>, Error = io::Error>>;

#[derive(Debug, Clone)]
/// Params needed to establish secured connection using Noise Protocol.
pub struct HandshakeParams {
    pub public_key: PublicKey,
    pub secret_key: SecretKey,
    pub max_message_len: u32,
}

pub trait Handshake {
    fn listen(self, params: &HandshakeParams, stream: TcpStream) -> HandshakeResult;
    fn send(self, params: &HandshakeParams, stream: TcpStream) -> HandshakeResult;
}

// may need refactoring (e.g., are `HandshakeParams` specific for the `NoiseHandshake`?
// if yes, `HandshakeParams` could be passed to constructor of `NoiseHandshake` instead of them
// being a part of `listen` / `send` methods
#[derive(Debug)]
pub struct NoiseHandshake {}

impl NoiseHandshake {
    pub fn new() -> Self {
        NoiseHandshake {}
    }
}

impl Handshake for NoiseHandshake {
    // stream could be abstracted to `AsyncRead + AsyncWrite + 'static`
    fn listen(self, params: &HandshakeParams, stream: TcpStream) -> HandshakeResult {
        let channel = NoiseHandshakeChannel {
            noise: NoiseWrapper::responder(params),
            max_message_len: params.max_message_len,
        };
        listen_handshake(stream, channel)
    }

    fn send(self, params: &HandshakeParams, stream: TcpStream) -> HandshakeResult {
        let channel = NoiseHandshakeChannel {
            noise: NoiseWrapper::initiator(params),
            max_message_len: params.max_message_len,
        };
        send_handshake(stream, channel)
    }
}

trait HandshakeChannel {
    fn read_handshake_msg<S: AsyncRead + 'static>(
        self,
        stream: S,
    ) -> Box<Future<Item = (S, Self), Error = io::Error>>;

    fn write_handshake_msg<S: AsyncWrite + 'static>(
        self,
        stream: S,
    ) -> Box<Future<Item = (S, Self), Error = io::Error>>;

    fn finalize<S: AsyncRead + AsyncWrite + 'static>(
        self,
        stream: S,
    ) -> Result<Framed<S, MessagesCodec>, io::Error>;
}

#[derive(Debug)]
struct NoiseHandshakeChannel {
    noise: NoiseWrapper,
    max_message_len: u32,
}

impl HandshakeChannel for NoiseHandshakeChannel {
    fn read_handshake_msg<S: AsyncRead + 'static>(
        mut self,
        stream: S,
    ) -> Box<Future<Item = (S, Self), Error = io::Error>> {
        Box::new(read(stream).and_then(move |(stream, msg)| {
            self.noise.read_handshake_msg(&msg)?;
            Ok((stream, self))
        }))
    }

    fn write_handshake_msg<S: AsyncWrite + 'static>(
        mut self,
        stream: S,
    ) -> Box<Future<Item = (S, Self), Error = io::Error>> {
        Box::new(
            done(self.noise.write_handshake_msg())
                .map_err(|e| e.into())
                .and_then(|(len, buf)| write(stream, &buf, len))
                .map(move |(stream, _)| (stream, self)),
        )
    }

    fn finalize<S: AsyncRead + AsyncWrite + 'static>(
        self,
        stream: S,
    ) -> Result<Framed<S, MessagesCodec>, io::Error> {
        let noise = self.noise.into_transport_mode()?;
        let framed = stream.framed(MessagesCodec::new(self.max_message_len, noise));
        Ok(framed)
    }
}

// may be made generic on `stream`
fn listen_handshake<T>(stream: TcpStream, channel: T) -> HandshakeResult
where
    T: HandshakeChannel + 'static,
{
    let framed = channel
        .read_handshake_msg(stream)
        .and_then(|(stream, channel)| channel.write_handshake_msg(stream))
        .and_then(|(stream, channel)| channel.read_handshake_msg(stream))
        .and_then(|(stream, channel)| channel.finalize(stream));
    Box::new(framed)
}

// may be made generic on `stream`
fn send_handshake<T>(stream: TcpStream, channel: T) -> HandshakeResult
where
    T: HandshakeChannel + 'static,
{
    let framed = channel
        .write_handshake_msg(stream)
        .and_then(|(stream, channel)| channel.read_handshake_msg(stream))
        .and_then(|(stream, channel)| channel.write_handshake_msg(stream))
        .and_then(|(stream, channel)| channel.finalize(stream));
    Box::new(framed)
}

fn read<S: AsyncRead + 'static>(sock: S) -> impl Future<Item = (S, Vec<u8>), Error = io::Error> {
    let buf = vec![0u8; HANDSHAKE_HEADER_LENGTH];
    read_exact(sock, buf).and_then(|(stream, msg)| read_exact(stream, vec![0u8; msg[0] as usize]))
}

fn write<S: AsyncWrite + 'static>(
    sock: S,
    buf: &[u8],
    len: usize,
) -> Box<Future<Item = (S, Vec<u8>), Error = io::Error>> {
    // should be changed into `impl Future`
    let mut message = vec![0u8; HANDSHAKE_HEADER_LENGTH];

    if len > NOISE_MAX_HANDSHAKE_MESSAGE_LENGTH {
        return Box::new(err(io::Error::new(
            io::ErrorKind::Other,
            "Message size exceeds max handshake message size",
        )));
    }

    LittleEndian::write_u16(&mut message, len as u16);
    message.extend_from_slice(&buf[0..len]);
    Box::new(write_all(sock, message))
}
