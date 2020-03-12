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

use anyhow::bail;
use async_trait::async_trait;
use exonum::{
    crypto::{
        x25519::{self, into_x25519_keypair, into_x25519_public_key},
        KeyPair, PublicKey,
    },
    merkledb::BinaryValue,
    messages::Verified,
};
use tokio::io::{AsyncRead, AsyncWrite};

use super::wrapper::NoiseWrapper;
use crate::{
    events::{
        codec::MessagesCodec,
        noise::{Handshake, HandshakeData, HandshakeRawMessage},
    },
    messages::Connect,
    state::SharedConnectList,
};

/// Params needed to establish secured connection using Noise Protocol.
#[derive(Debug, Clone)]
pub struct HandshakeParams {
    pub(super) secret_key: x25519::SecretKey,
    pub(super) remote_key: Option<x25519::PublicKey>,
    pub(crate) connect_list: SharedConnectList,
    pub(crate) connect: Verified<Connect>,
    max_message_len: u32,
}

impl HandshakeParams {
    pub(crate) fn new(
        keypair: &KeyPair,
        connect_list: SharedConnectList,
        connect: Verified<Connect>,
        max_message_len: u32,
    ) -> Self {
        let (_, secret_key) =
            into_x25519_keypair(keypair.public_key(), keypair.secret_key().to_owned()).unwrap();

        Self {
            secret_key,
            max_message_len,
            remote_key: None,
            connect,
            connect_list,
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
    connect: Verified<Connect>,
}

impl NoiseHandshake {
    pub fn initiator(params: &HandshakeParams) -> Self {
        let noise = NoiseWrapper::initiator(params);
        Self {
            noise,
            max_message_len: params.max_message_len,
            connect_list: params.connect_list.clone(),
            connect: params.connect.clone(),
        }
    }

    pub fn responder(params: &HandshakeParams) -> Self {
        let noise = NoiseWrapper::responder(params);
        Self {
            noise,
            max_message_len: params.max_message_len,
            connect_list: params.connect_list.clone(),
            connect: params.connect.clone(),
        }
    }

    pub async fn read_handshake_msg<S>(&mut self, stream: &mut S) -> anyhow::Result<Vec<u8>>
    where
        S: AsyncRead + Unpin,
    {
        let handshake = HandshakeRawMessage::read(stream).await?;
        let message = self.noise.read_handshake_msg(&handshake.0)?;
        Ok(message)
    }

    pub async fn write_handshake_msg<S>(&mut self, stream: &mut S, msg: &[u8]) -> anyhow::Result<()>
    where
        S: AsyncWrite + Unpin,
    {
        let buf = self.noise.write_handshake_msg(msg)?;
        HandshakeRawMessage(buf).write(stream).await
    }

    pub fn finalize(self, raw_message: Vec<u8>) -> anyhow::Result<HandshakeData> {
        let peer_key = {
            // Panic because with selected handshake pattern we must have
            // `remote_static_key` on final step of handshake.
            let rs = self
                .noise
                .state
                .get_remote_static()
                .expect("Remote static key is not present");
            x25519::PublicKey::from_slice(rs).expect("Remote static key is not valid x25519 key")
        };

        if !self.is_peer_allowed(&peer_key) {
            bail!("peer is not in ConnectList");
        }

        let noise = self.noise.into_transport_wrapper()?;
        let codec = MessagesCodec::new(self.max_message_len, noise);
        Ok(HandshakeData {
            codec,
            raw_message,
            peer_key,
        })
    }

    fn is_peer_allowed(&self, remote_static_key: &x25519::PublicKey) -> bool {
        self.connect_list
            .peers()
            .iter()
            .map(|info| into_x25519_public_key(info.public_key))
            .any(|key| remote_static_key == &key)
    }
}

#[async_trait]
impl<S> Handshake<S> for NoiseHandshake
where
    S: AsyncRead + AsyncWrite + Send + Unpin,
{
    async fn listen(mut self, stream: &mut S) -> anyhow::Result<HandshakeData> {
        self.read_handshake_msg(stream).await?;
        self.write_handshake_msg(stream, &self.connect.to_bytes())
            .await?;
        let raw_message = self.read_handshake_msg(stream).await?;
        self.finalize(raw_message)
    }

    async fn send(mut self, stream: &mut S) -> anyhow::Result<HandshakeData> {
        self.write_handshake_msg(stream, &[]).await?;
        let message = self.read_handshake_msg(stream).await?;
        self.write_handshake_msg(stream, &self.connect.to_bytes())
            .await?;
        self.finalize(message)
    }
}
