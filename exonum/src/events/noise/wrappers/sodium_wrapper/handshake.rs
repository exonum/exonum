// Copyright 2019 The Exonum Team
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
use tokio_codec::{Decoder, Framed};
use tokio_io::{AsyncRead, AsyncWrite};

use std::net::SocketAddr;

use exonum_merkledb::BinaryValue;

use super::wrapper::NoiseWrapper;
use crate::{
    crypto::{PublicKeyKx, SecretKeyKx},
    events::{
        codec::MessagesCodec,
        noise::{Handshake, HandshakeRawMessage, HandshakeResult},
    },
    messages::{Connect, Signed},
    node::state::SharedConnectList,
};

/// Params needed to establish secured connection using Noise Protocol.
#[derive(Debug, Clone)]
pub struct HandshakeParams {
    pub public_key: PublicKeyKx,
    pub secret_key: SecretKeyKx,
    pub remote_key: Option<PublicKeyKx>,
    pub connect_list: SharedConnectList,
    pub connect: Signed<Connect>,
    max_message_len: u32,
}

impl HandshakeParams {
    pub fn new(
        public_key: PublicKeyKx,
        secret_key: SecretKeyKx,
        connect_list: SharedConnectList,
        connect: Signed<Connect>,
        max_message_len: u32,
    ) -> Self {
        HandshakeParams {
            public_key,
            secret_key,
            max_message_len,
            remote_key: None,
            connect,
            connect_list,
        }
    }

    pub fn set_remote_key(&mut self, remote_key: PublicKeyKx) {
        self.remote_key = Some(remote_key);
    }
}

#[derive(Debug)]
pub struct NoiseHandshake {
    noise: NoiseWrapper,
    peer_address: SocketAddr,
    max_message_len: u32,
    connect_list: SharedConnectList,
    connect: Signed<Connect>,
}

impl NoiseHandshake {
    pub fn initiator(params: &HandshakeParams, peer_address: &SocketAddr) -> Self {
        let noise = NoiseWrapper::initiator(params);
        NoiseHandshake {
            noise,
            peer_address: *peer_address,
            max_message_len: params.max_message_len,
            connect_list: params.connect_list.clone(),
            connect: params.connect.clone(),
        }
    }

    pub fn responder(params: &HandshakeParams, peer_address: &SocketAddr) -> Self {
        let noise = NoiseWrapper::responder(params);
        NoiseHandshake {
            noise,
            peer_address: *peer_address,
            max_message_len: params.max_message_len,
            connect_list: params.connect_list.clone(),
            connect: params.connect.clone(),
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
            .map_err(Into::into)
            .and_then(|buf| HandshakeRawMessage(buf).write(stream))
            .map(move |(stream, _)| (stream, self))
    }

    pub fn finalize<S: AsyncRead + AsyncWrite + 'static>(
        self,
        stream: S,
        message: Vec<u8>,
    ) -> Result<(Framed<S, MessagesCodec>, Vec<u8>), failure::Error> {
        let remote_static_key = {
            // Panic because with selected handshake pattern we must have
            // `remote_static_key` on final step of handshake.
            let rs = self
                .noise
                .state
                .get_remote_static()
                .expect("Remote static key is not present!");
            PublicKeyKx::from_slice(rs).expect("Remote static key is not valid x25519 key!")
        };

        if !self.is_peer_allowed(&remote_static_key) {
            bail!("peer is not in ConnectList")
        }

        let noise = self.noise.into_transport_wrapper()?;
        let framed = MessagesCodec::new(self.max_message_len, noise).framed(stream);
        Ok((framed, message))
    }

    fn is_peer_allowed(&self, remote_static_key: &PublicKeyKx) -> bool {
        self.connect_list
            .peers()
            .iter()
            .map(|info| info.identity_key)
            .any(|key| {
                if let Some(key) = key {
                    remote_static_key == &key
                } else {
                    false
                }
            })
    }
}

impl Handshake for NoiseHandshake {
    fn listen<S>(self, stream: S) -> HandshakeResult<S>
    where
        S: AsyncRead + AsyncWrite + 'static,
    {
        let peer_address = self.peer_address;
        let connect = self.connect.clone();
        let framed = self
            .read_handshake_msg(stream)
            .and_then(|(stream, handshake, _)| {
                handshake.write_handshake_msg(stream, &connect.into_bytes())
            })
            .and_then(|(stream, handshake)| handshake.read_handshake_msg(stream))
            .and_then(|(stream, handshake, message)| handshake.finalize(stream, message))
            .map_err(move |e| {
                e.context(format!("peer {} disconnected", peer_address))
                    .into()
            });
        Box::new(framed)
    }

    fn send<S>(self, stream: S) -> HandshakeResult<S>
    where
        S: AsyncRead + AsyncWrite + 'static,
    {
        let peer_address = self.peer_address;
        let connect = self.connect.clone();
        let framed = self
            .write_handshake_msg(stream, &[])
            .and_then(|(stream, handshake)| handshake.read_handshake_msg(stream))
            .and_then(|(stream, handshake, message)| {
                (
                    handshake.write_handshake_msg(stream, &connect.into_bytes()),
                    Ok(message),
                )
            })
            .and_then(|((stream, handshake), message)| handshake.finalize(stream, message))
            .map_err(move |e| {
                e.context(format!("peer {} disconnected", peer_address))
                    .into()
            });
        Box::new(framed)
    }
}
