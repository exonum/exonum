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

// spell-checker:ignore uint

#[cfg(feature = "exonum_sodiumoxide")]
pub use self::wrappers::sodium_wrapper::{
    handshake::{HandshakeParams, NoiseHandshake},
    wrapper::{
        NoiseWrapper, TransportWrapper, HANDSHAKE_HEADER_LENGTH, MAX_HANDSHAKE_MESSAGE_LENGTH,
        MIN_HANDSHAKE_MESSAGE_LENGTH,
    },
};

use async_trait::async_trait;
use byteorder::{ByteOrder, LittleEndian};
use exonum::crypto::x25519;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::events::codec::MessagesCodec;

pub mod error;
pub mod wrappers;

#[cfg(test)]
mod tests;

pub const MAX_MESSAGE_LENGTH: usize = 65_535;
pub const TAG_LENGTH: usize = 16;
pub const HEADER_LENGTH: usize = 4;

#[derive(Debug)]
pub struct HandshakeData {
    pub codec: MessagesCodec,
    pub raw_message: Vec<u8>,
    pub peer_key: x25519::PublicKey,
}

#[async_trait]
pub trait Handshake<S> {
    async fn listen(self, stream: &mut S) -> anyhow::Result<HandshakeData>;
    async fn send(self, stream: &mut S) -> anyhow::Result<HandshakeData>;
}

pub struct HandshakeRawMessage(pub Vec<u8>);

impl HandshakeRawMessage {
    pub async fn read<S>(sock: &mut S) -> anyhow::Result<Self>
    where
        S: AsyncRead + Unpin,
    {
        let mut len_buf = [0_u8; HANDSHAKE_HEADER_LENGTH];
        // First `HANDSHAKE_HEADER_LENGTH` bytes of handshake message is the payload length
        // in little-endian, remaining bytes is the handshake payload. Therefore, we need to read
        // `HANDSHAKE_HEADER_LENGTH` bytes as a little-endian integer and than we need to read
        // remaining payload.
        sock.read_exact(&mut len_buf).await?;
        let len = LittleEndian::read_uint(&len_buf, HANDSHAKE_HEADER_LENGTH);
        let mut message = vec![0_u8; len as usize];
        sock.read_exact(&mut message).await?;
        Ok(Self(message))
    }

    pub async fn write<S>(&self, sock: &mut S) -> anyhow::Result<()>
    where
        S: AsyncWrite + Unpin,
    {
        let len = self.0.len();
        debug_assert!(len < MAX_HANDSHAKE_MESSAGE_LENGTH);

        // First `HANDSHAKE_HEADER_LENGTH` bytes of handshake message
        // is the payload length in little-endian.
        let mut len_buf = [0_u8; HANDSHAKE_HEADER_LENGTH];
        LittleEndian::write_uint(&mut len_buf, len as u64, HANDSHAKE_HEADER_LENGTH);
        sock.write_all(&len_buf).await?;
        sock.write_all(&self.0).await.map_err(From::from)
    }
}
