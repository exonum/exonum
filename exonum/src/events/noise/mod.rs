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

use futures::future::{done, Future};
use tokio_io::{
    codec::Framed, io::{read_exact, write_all}, AsyncRead, AsyncWrite,
};

use failure;

use crypto::{
    x25519::{self, into_x25519_keypair, into_x25519_public_key}, PublicKey, SecretKey,
};
use events::noise::wrapper::NOISE_MAX_HANDSHAKE_MESSAGE_LENGTH;
use events::{
    codec::MessagesCodec, error::into_failure,
    noise::wrapper::{NoiseWrapper, HANDSHAKE_HEADER_LENGTH},
};

pub mod sodium_resolver;
pub mod wrapper;

#[cfg(test)]
mod tests;

type HandshakeResult<S> = Box<dyn Future<Item = Framed<S, MessagesCodec>, Error = failure::Error>>;

#[derive(Debug, Clone)]
/// Params needed to establish secured connection using Noise Protocol.
pub struct HandshakeParams {
    pub public_key: x25519::PublicKey,
    pub secret_key: x25519::SecretKey,
    pub max_message_len: u32,
    pub remote_key: Option<x25519::PublicKey>,
}

impl HandshakeParams {
    pub fn new(public_key: PublicKey, secret_key: SecretKey, max_message_len: u32) -> Self {
        let (public_key, secret_key) = into_x25519_keypair(public_key, secret_key).unwrap();

        Self {
            public_key,
            secret_key,
            max_message_len,
            remote_key: None,
        }
    }

    pub fn set_remote_key(&mut self, remote_key: PublicKey) {
        self.remote_key = Some(into_x25519_public_key(remote_key));
    }
}

pub trait Handshake {
    fn listen<S: AsyncRead + AsyncWrite + 'static>(self, stream: S) -> HandshakeResult<S>;
    fn send<S: AsyncRead + AsyncWrite + 'static>(self, stream: S) -> HandshakeResult<S>;
}

#[derive(Debug)]
pub struct NoiseHandshake {
    noise: NoiseWrapper,
    max_message_len: u32,
}

impl NoiseHandshake {
    pub fn initiator(params: &HandshakeParams) -> Self {
        let noise = NoiseWrapper::initiator(params);
        Self {
            noise,
            max_message_len: params.max_message_len,
        }
    }

    pub fn responder(params: &HandshakeParams) -> Self {
        let noise = NoiseWrapper::responder(params);
        Self {
            noise,
            max_message_len: params.max_message_len,
        }
    }

    fn read_handshake_msg<S: AsyncRead + 'static>(
        mut self,
        stream: S,
    ) -> impl Future<Item = (S, Self), Error = failure::Error> {
        read(stream).and_then(move |(stream, msg)| {
            self.noise.read_handshake_msg(&msg)?;
            Ok((stream, self))
        })
    }

    fn write_handshake_msg<S: AsyncWrite + 'static>(
        mut self,
        stream: S,
    ) -> impl Future<Item = (S, Self), Error = failure::Error> {
        done(self.noise.write_handshake_msg())
            .map_err(|e| e.into())
            .and_then(|(len, buf)| write(stream, &buf, len))
            .map(move |(stream, _)| (stream, self))
    }

    fn finalize<S: AsyncRead + AsyncWrite + 'static>(
        self,
        stream: S,
    ) -> Result<Framed<S, MessagesCodec>, failure::Error> {
        let noise = self.noise.into_transport_mode()?;
        let framed = stream.framed(MessagesCodec::new(self.max_message_len, noise));
        Ok(framed)
    }
}

impl Handshake for NoiseHandshake {
    fn listen<S>(self, stream: S) -> HandshakeResult<S>
    where
        S: AsyncRead + AsyncWrite + 'static,
    {
        let framed = self.read_handshake_msg(stream)
            .and_then(|(stream, handshake)| handshake.write_handshake_msg(stream))
            .and_then(|(stream, handshake)| handshake.read_handshake_msg(stream))
            .and_then(|(stream, handshake)| handshake.finalize(stream));
        Box::new(framed)
    }

    fn send<S>(self, stream: S) -> HandshakeResult<S>
    where
        S: AsyncRead + AsyncWrite + 'static,
    {
        let framed = self.write_handshake_msg(stream)
            .and_then(|(stream, handshake)| handshake.read_handshake_msg(stream))
            .and_then(|(stream, handshake)| handshake.write_handshake_msg(stream))
            .and_then(|(stream, handshake)| handshake.finalize(stream));
        Box::new(framed)
    }
}

fn read<S: AsyncRead + 'static>(
    sock: S,
) -> impl Future<Item = (S, Vec<u8>), Error = failure::Error> {
    let buf = vec![0_u8; HANDSHAKE_HEADER_LENGTH];
    // First byte of handshake message is payload length, remaining bytes [1; len] is
    // the handshake payload. Therefore, we need to read first byte and after that
    // remaining payload.
    read_exact(sock, buf)
        .and_then(|(stream, msg)| read_exact(stream, vec![0_u8; msg[0] as usize]))
        .map_err(into_failure)
}

fn write<S: AsyncWrite + 'static>(
    sock: S,
    buf: &[u8],
    len: usize,
) -> impl Future<Item = (S, Vec<u8>), Error = failure::Error> {
    debug_assert!(len < NOISE_MAX_HANDSHAKE_MESSAGE_LENGTH);

    let mut message = vec![len as u8; HANDSHAKE_HEADER_LENGTH];
    message.extend_from_slice(&buf[0..len]);
    write_all(sock, message).map_err(into_failure)
}
