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

use failure;
use futures::future::{done, Future};
use tokio_codec::{Decoder, Framed};
use tokio_io::{AsyncRead, AsyncWrite};

use std::{io, net::SocketAddr};

use super::wrapper::NoiseWrapper;
use crypto::{
    x25519::{self, into_x25519_keypair, into_x25519_public_key}, PublicKey, SecretKey,
};
use events::{
    codec::MessagesCodec, noise::{Handshake, HandshakeRawMessage, HandshakeResult},
};
use futures::future;
use messages::PeersExchange;
use node::{state::SharedConnectList, ConnectInfo};
use storage::StorageValue;

/// Params needed to establish secured connection using Noise Protocol.
#[derive(Debug, Clone)]
pub struct HandshakeParams {
    pub connect_list: SharedConnectList,
    connect_info: ConnectInfo,
    public_key: PublicKey,
    secret_key: SecretKey,
    remote_key: Option<PublicKey>,
    public_key_x25519: x25519::PublicKey,
    secret_key_x25519: x25519::SecretKey,
    remote_key_x25519: Option<x25519::PublicKey>,
    max_message_len: u32,
}

impl HandshakeParams {
    pub fn new(
        public_key: PublicKey,
        secret_key: SecretKey,
        connect_list: SharedConnectList,
        max_message_len: u32,
        address: SocketAddr,
    ) -> Self {
        let (public_key_x25519, secret_key_x25519) =
            into_x25519_keypair(public_key, secret_key.clone()).unwrap();

        HandshakeParams {
            public_key_x25519,
            secret_key_x25519,
            max_message_len,
            remote_key: None,
            remote_key_x25519: None,
            connect_list,
            connect_info: ConnectInfo {
                address,
                public_key,
            },
            public_key,
            secret_key,
        }
    }

    pub fn set_remote_key(&mut self, remote_key: PublicKey) {
        self.remote_key = Some(remote_key);
        self.remote_key_x25519 = Some(into_x25519_public_key(remote_key));
    }

    pub fn peers_exchange(&self) -> Option<PeersExchange> {
        self.remote_key.map(|key| {
            PeersExchange::new(
                &self.public_key,
                &key,
                vec![self.connect_info],
                &self.secret_key,
            )
        })
    }

    pub fn public_key(&self) -> &x25519::PublicKey {
        &self.public_key_x25519
    }

    pub fn secret_key(&self) -> &x25519::SecretKey {
        &self.secret_key_x25519
    }

    pub fn remote_key(&self) -> &Option<x25519::PublicKey> {
        &self.remote_key_x25519
    }

    pub fn connect_list(&self) -> SharedConnectList {
        self.connect_list.clone()
    }
}

#[derive(Debug)]
pub struct NoiseHandshake {
    noise: NoiseWrapper,
    peer_address: SocketAddr,
    max_message_len: u32,
    connect_list: SharedConnectList,
    peers_exchange: Option<PeersExchange>,
}

impl NoiseHandshake {
    pub fn initiator(params: &HandshakeParams, peer_address: &SocketAddr) -> Self {
        let noise = NoiseWrapper::initiator(params);
        NoiseHandshake {
            noise,
            peer_address: *peer_address,
            max_message_len: params.max_message_len,
            connect_list: params.connect_list.clone(),
            peers_exchange: params.peers_exchange(),
        }
    }

    pub fn responder(params: &HandshakeParams, peer_address: &SocketAddr) -> Self {
        let noise = NoiseWrapper::responder(params);
        NoiseHandshake {
            noise,
            peer_address: *peer_address,
            max_message_len: params.max_message_len,
            connect_list: params.connect_list.clone(),
            peers_exchange: params.peers_exchange(),
        }
    }

    pub fn read_handshake_msg<S: AsyncRead + 'static>(
        mut self,
        stream: S,
    ) -> impl Future<Item = (S, Self, Vec<u8>), Error = failure::Error> {
        HandshakeRawMessage::read(stream).and_then(move |(stream, msg)| {
            let message = self.noise.read_handshake_msg(&msg.0)?;
            Ok((stream, self, message))
        })
    }

    pub fn write_handshake_msg<S: AsyncWrite + 'static>(
        mut self,
        stream: S,
        msg: &[u8],
    ) -> impl Future<Item = (S, Self), Error = failure::Error> {
        done(self.noise.write_handshake_msg(msg))
            .map_err(|e| e.into())
            .and_then(|buf| HandshakeRawMessage(buf).write(stream))
            .map(move |(stream, _)| (stream, self))
    }

    pub fn finalize<S: AsyncRead + AsyncWrite + 'static>(
        self,
        stream: S,
        peers_exchange: Vec<u8>,
    ) -> Result<(Framed<S, MessagesCodec>, Vec<u8>), failure::Error> {
        let remote_static_key = {
            // Panic because with selected handshake pattern we must have
            // `remote_static_key` on final step of handshake.
            let rs = self.noise
                .session
                .get_remote_static()
                .expect("Remote static key is not present!");
            x25519::PublicKey::from_slice(rs).expect("Remote static key is not valid x25519 key!")
        };

        if !self.is_peer_allowed(&remote_static_key) {
            bail!("peer is not in ConnectList")
        }

        let noise = self.noise.into_transport_mode()?;
        let framed = MessagesCodec::new(self.max_message_len, noise).framed(stream);
        Ok((framed, peers_exchange))
    }

    fn is_peer_allowed(&self, remote_static_key: &x25519::PublicKey) -> bool {
        self.connect_list
            .peers()
            .iter()
            .map(|info| into_x25519_public_key(info.public_key))
            .any(|key| remote_static_key == &key)
    }
}

impl Handshake for NoiseHandshake {
    type Result = Vec<u8>;

    fn listen<S>(self, stream: S) -> HandshakeResult<S, Self::Result>
    where
        S: AsyncRead + AsyncWrite + 'static,
    {
        let peer_address = self.peer_address;
        let framed = self.read_handshake_msg(stream)
            .and_then(|(stream, handshake, _)| handshake.write_handshake_msg(stream, &[]))
            .and_then(|(stream, handshake)| handshake.read_handshake_msg(stream))
            .and_then(|(stream, handshake, peers_exchange)| {
                handshake.finalize(stream, peers_exchange)
            })
            .map_err(move |e| {
                e.context(format!("peer {} disconnected", peer_address))
                    .into()
            });
        Box::new(framed)
    }

    fn send<S>(self, stream: S) -> HandshakeResult<S, Self::Result>
    where
        S: AsyncRead + AsyncWrite + 'static,
    {
        match self.peers_exchange.clone() {
            Some(peers_exchange) => {
                let framed = self.write_handshake_msg(stream, &[])
                    .and_then(|(stream, handshake)| handshake.read_handshake_msg(stream))
                    .and_then(move |(stream, handshake, _)| {
                        handshake.write_handshake_msg(stream, &peers_exchange.into_bytes())
                    })
                    .and_then(|(stream, handshake)| handshake.finalize(stream, Vec::new()))
                    .map_err(move |e| {
                    e.context(format!("peer {} disconnected", peer_address))
                        .into()
                });
                Box::new(framed)
            }
            None => Box::new(future::err(other_error(
                "Can't send handshake request without PeersExchange",
            ))),
        }
    }
}
