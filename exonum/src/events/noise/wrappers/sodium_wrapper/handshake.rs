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

use super::wrapper::NoiseWrapper;
use crypto::{
    x25519::{self, into_x25519_keypair, into_x25519_public_key},
    PublicKey,
    SecretKey
};
use events::{
    codec::MessagesCodec,
    noise::{read, write, Handshake, HandshakeResult}
};
use futures::future::{done, Future};
use std::io;
use tokio_io::{codec::Framed, AsyncRead, AsyncWrite};

/// Params needed to establish secured connection using Noise Protocol.
#[derive(Debug, Clone)]
pub struct HandshakeParams {
    pub public_key: x25519::PublicKey,
    pub secret_key: x25519::SecretKey,
    pub max_message_len: u32,
    pub remote_key: Option<x25519::PublicKey>,
}

impl HandshakeParams {
    pub fn new(public_key: PublicKey, secret_key: SecretKey, max_message_len: u32) -> Self {
        let (public_key, secret_key) = into_x25519_keypair(public_key, secret_key).unwrap();

        HandshakeParams {
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

#[derive(Debug)]
pub struct NoiseHandshake {
    noise: NoiseWrapper,
    max_message_len: u32,
}

impl NoiseHandshake {
    pub fn initiator(params: &HandshakeParams) -> Self {
        let noise = NoiseWrapper::initiator(params);
        NoiseHandshake {
            noise,
            max_message_len: params.max_message_len,
        }
    }

    pub fn responder(params: &HandshakeParams) -> Self {
        let noise = NoiseWrapper::responder(params);
        NoiseHandshake {
            noise,
            max_message_len: params.max_message_len,
        }
    }

    pub fn read_handshake_msg<S: AsyncRead + 'static>(
        mut self,
        stream: S,
    ) -> impl Future<Item = (S, Self), Error = io::Error> {
        read(stream).and_then(move |(stream, msg)| {
            self.noise.read_handshake_msg(&msg)?;
            Ok((stream, self))
        })
    }

    pub fn write_handshake_msg<S: AsyncWrite + 'static>(
        mut self,
        stream: S,
    ) -> impl Future<Item = (S, Self), Error = io::Error> {
        done(self.noise.write_handshake_msg())
            .map_err(|e| e.into())
            .and_then(|(len, buf)| write(stream, &buf, len))
            .map(move |(stream, _)| (stream, self))
    }

    pub fn finalize<S: AsyncRead + AsyncWrite + 'static>(
        self,
        stream: S,
    ) -> Result<Framed<S, MessagesCodec>, io::Error> {
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
