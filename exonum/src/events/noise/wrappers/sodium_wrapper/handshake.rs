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
use tokio_codec::{Decoder, Framed};
use tokio_io::{AsyncRead, AsyncWrite};

use std::io;

use super::wrapper::NoiseWrapper;
use crypto::{
    x25519::{self, into_x25519_keypair, into_x25519_public_key}, PublicKey, SecretKey,
};
use events::{
    codec::MessagesCodec, error::other_error,
    noise::{Handshake, HandshakeRawMessage, HandshakeResult},
};
use node::state::SharedConnectList;
use node::ConnectInfo;
use std::net::SocketAddr;

/// Params needed to establish secured connection using Noise Protocol.
#[derive(Debug, Clone)]
pub struct HandshakeParams {
    pub public_key: x25519::PublicKey,
    pub secret_key: x25519::SecretKey,
    pub remote_key: Option<x25519::PublicKey>,
    pub connect_list: SharedConnectList,
    pub connect_info: ConnectInfo,
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
            into_x25519_keypair(public_key, secret_key).unwrap();

        HandshakeParams {
            public_key: public_key_x25519,
            secret_key: secret_key_x25519,
            max_message_len,
            remote_key: None,
            connect_list,
            connect_info: ConnectInfo {
                address,
                public_key,
            },
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
    connect_list: SharedConnectList,
    connect_info: ConnectInfo,
}

impl NoiseHandshake {
    pub fn initiator(params: &HandshakeParams) -> Self {
        let noise = NoiseWrapper::initiator(params);
        NoiseHandshake {
            noise,
            max_message_len: params.max_message_len,
            connect_list: params.connect_list.clone(),
            connect_info: params.connect_info,
        }
    }

    pub fn responder(params: &HandshakeParams) -> Self {
        let noise = NoiseWrapper::responder(params);
        NoiseHandshake {
            noise,
            max_message_len: params.max_message_len,
            connect_list: params.connect_list.clone(),
            connect_info: params.connect_info,
        }
    }

    pub fn read_handshake_msg<S: AsyncRead + 'static>(
        mut self,
        stream: S,
    ) -> impl Future<Item = (S, Self, Option<ConnectInfo>), Error = io::Error> {
        HandshakeRawMessage::read(stream).and_then(move |(stream, msg)| {
            let message = self.noise.read_handshake_msg(&msg.0)?;
            let remote_connect_info = ConnectInfo::try_deserialize(message.as_ref());
            Ok((stream, self, remote_connect_info.ok()))
        })
    }

    pub fn write_handshake_msg<S: AsyncWrite + 'static>(
        mut self,
        stream: S,
        msg: &[u8],
    ) -> impl Future<Item = (S, Self), Error = io::Error> {
        done(self.noise.write_handshake_msg(msg))
            .map_err(|e| e.into())
            .and_then(|buf| HandshakeRawMessage(buf).write(stream))
            .map(move |(stream, _)| (stream, self))
    }

    pub fn finalize<S: AsyncRead + AsyncWrite + 'static>(
        self,
        stream: S,
        info: Option<ConnectInfo>,
    ) -> Result<(Framed<S, MessagesCodec>, Option<ConnectInfo>), io::Error> {
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
            return Err(other_error("Peer is not in ConnectList"));
        }

        let noise = self.noise.into_transport_mode()?;
        let framed = MessagesCodec::new(self.max_message_len, noise).framed(stream);
        Ok((framed, info))
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
    type Result = Option<ConnectInfo>;

    fn listen<S>(self, stream: S) -> HandshakeResult<S, Self::Result>
    where
        S: AsyncRead + AsyncWrite + 'static,
    {
        let framed = self.read_handshake_msg(stream)
            .and_then(|(stream, handshake, _)| handshake.write_handshake_msg(stream, &[]))
            .and_then(|(stream, handshake)| handshake.read_handshake_msg(stream))
            .and_then(|(stream, handshake, info)| handshake.finalize(stream, info));
        Box::new(framed)
    }

    fn send<S>(self, stream: S) -> HandshakeResult<S, Self::Result>
    where
        S: AsyncRead + AsyncWrite + 'static,
    {
        let connect_info = self.connect_info;
        let framed = self.write_handshake_msg(stream, &[])
            .and_then(|(stream, handshake)| handshake.read_handshake_msg(stream))
            .and_then(move |(stream, handshake, _)| {
                handshake.write_handshake_msg(stream, &connect_info.try_serialize().unwrap())
            })
            .and_then(|(stream, handshake)| handshake.finalize(stream, None));
        Box::new(framed)
    }
}
