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

use byteorder::{ByteOrder, LittleEndian};
use bytes::BytesMut;
use snow::{Builder, HandshakeState, TransportState};

use std::fmt::{self, Error, Formatter};

use super::{handshake::HandshakeParams, resolver::SodiumResolver};
use crate::events::noise::{error::NoiseError, HEADER_LENGTH, MAX_MESSAGE_LENGTH, TAG_LENGTH};

// Maximum allowed handshake message length is 65535,
// therefore HANDSHAKE_HEADER_LENGTH cannot exceed 2.
pub const HANDSHAKE_HEADER_LENGTH: usize = 2;
pub const MAX_HANDSHAKE_MESSAGE_LENGTH: usize = 65535;
pub const MIN_HANDSHAKE_MESSAGE_LENGTH: usize = 32;

// We choose XK pattern since it provides mutual authentication,
// transmission of static public keys and requires pre-defined remote public
// key to establish connection.
// See: https://noiseprotocol.org/noise.html#interactive-patterns
static PARAMS: &str = "Noise_XK_25519_ChaChaPoly_SHA256";

/// Wrapper around noise handshake state to provide latter convenient interface.
pub struct NoiseWrapper {
    pub state: HandshakeState,
}

impl NoiseWrapper {
    pub fn initiator(params: &HandshakeParams) -> Self {
        if let Some(ref remote_key) = params.remote_key {
            let builder: Builder<'_> = Self::noise_builder()
                .local_private_key(params.secret_key.as_ref())
                .remote_public_key(remote_key.as_ref());
            let state = builder
                .build_initiator()
                .expect("Noise session initiator failed to initialize");
            Self { state }
        } else {
            panic!("Remote public key is not specified")
        }
    }

    pub fn responder(params: &HandshakeParams) -> Self {
        let builder: Builder<'_> = Self::noise_builder();

        let state = builder
            .local_private_key(params.secret_key.as_ref())
            .build_responder()
            .expect("Noise session responder failed to initialize");

        Self { state }
    }

    pub fn read_handshake_msg(&mut self, input: &[u8]) -> Result<Vec<u8>, NoiseError> {
        if input.len() < MIN_HANDSHAKE_MESSAGE_LENGTH || input.len() > MAX_MESSAGE_LENGTH {
            return Err(NoiseError::WrongMessageLength(input.len()));
        }

        let mut buf = vec![0_u8; MAX_MESSAGE_LENGTH];
        let len = self.state.read_message(input, &mut buf)?;
        buf.truncate(len);
        Ok(buf)
    }

    pub fn write_handshake_msg(&mut self, msg: &[u8]) -> Result<Vec<u8>, NoiseError> {
        let mut buf = vec![0_u8; MAX_MESSAGE_LENGTH];
        let len = self.state.write_message(msg, &mut buf)?;
        buf.truncate(len);
        Ok(buf)
    }

    pub fn into_transport_wrapper(self) -> Result<TransportWrapper, NoiseError> {
        // Transition into transport mode after handshake is finished.
        let state = self.state.into_transport_mode()?;
        Ok(TransportWrapper { state })
    }

    fn noise_builder<'a>() -> Builder<'a> {
        Builder::with_resolver(PARAMS.parse().unwrap(), Box::new(SodiumResolver::new()))
    }
}

impl fmt::Debug for NoiseWrapper {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(
            f,
            "NoiseWrapper {{ handshake finished: {} }}",
            self.state.is_handshake_finished()
        )
    }
}

/// Wrapper around noise transport state to provide latter convenient interface.
pub struct TransportWrapper {
    pub state: TransportState,
}

impl TransportWrapper {
    /// Decrypts `msg` using Noise session.
    ///
    /// Decryption consists of the following steps:
    /// 1. Message splits to packets of length smaller or equal to 65535 bytes.
    /// 2. Then each packet is decrypted by selected noise algorithm.
    /// 3. Append all decrypted packets to `decoded_message`.
    pub fn decrypt_msg(&mut self, len: usize, buf: &mut BytesMut) -> anyhow::Result<BytesMut> {
        debug_assert!(len + HEADER_LENGTH <= buf.len());
        let data = buf.split_to(len + HEADER_LENGTH).to_vec();
        let data = &data[HEADER_LENGTH..];

        let len = decrypted_msg_len(data.len());
        let mut decrypted_message = BytesMut::with_capacity(len);
        decrypted_message.resize(len, 0);

        let mut read = vec![0_u8; MAX_MESSAGE_LENGTH];
        for (i, msg) in data.chunks(MAX_MESSAGE_LENGTH).enumerate() {
            let len = self.state.read_message(msg, &mut read)?;
            let start = i * (MAX_MESSAGE_LENGTH - TAG_LENGTH);
            let end = start + len;

            decrypted_message[start..end].copy_from_slice(&read[..len]);
        }

        Ok(decrypted_message)
    }

    /// Encrypts `msg` using Noise session
    ///
    /// Encryption consists of the following steps:
    ///
    /// 1. Message splits to packets of length smaller or equal to 65535 bytes.
    /// 2. Then each packet is encrypted by selected noise algorithm.
    /// 3. Result message: first 4 bytes is message length(`len').
    /// 4. Append all encrypted packets in corresponding order.
    /// 5. Write result message to `buf`
    pub fn encrypt_msg(&mut self, msg: &[u8], buf: &mut BytesMut) -> anyhow::Result<()> {
        //TODO: don't use additional allocations [ECR-2213]
        const CHUNK_LENGTH: usize = MAX_MESSAGE_LENGTH - TAG_LENGTH;
        let len = encrypted_msg_len(msg.len());
        let mut encrypted_message = vec![0; len + HEADER_LENGTH];
        LittleEndian::write_u32(&mut encrypted_message[..HEADER_LENGTH], len as u32);

        let mut written = vec![0_u8; MAX_MESSAGE_LENGTH];
        for (i, msg) in msg.chunks(CHUNK_LENGTH).enumerate() {
            let len = self.state.write_message(msg, &mut written)?;
            let start = HEADER_LENGTH + i * MAX_MESSAGE_LENGTH;
            let end = start + len;

            encrypted_message[start..end].copy_from_slice(&written[..len]);
        }

        buf.extend_from_slice(&encrypted_message);
        Ok(())
    }
}

// Each message consists of the payload and 16 bytes(`TAG_LENGTH`)
// of AEAD authentication data. Therefore to calculate an actual message
// length we need to subtract `TAG_LENGTH` multiplied by messages count
// from `data.len()`.
fn decrypted_msg_len(raw_message_len: usize) -> usize {
    raw_message_len - TAG_LENGTH * div_ceil(raw_message_len, MAX_MESSAGE_LENGTH)
}

// In case of encryption we need to add `TAG_LENGTH` multiplied by messages count to
// calculate actual message length.
fn encrypted_msg_len(raw_message_len: usize) -> usize {
    let tag_count = div_ceil(raw_message_len, MAX_MESSAGE_LENGTH - TAG_LENGTH);
    raw_message_len + TAG_LENGTH * tag_count
}

fn div_ceil(lhs: usize, rhs: usize) -> usize {
    match (lhs / rhs, lhs % rhs) {
        (d, r) if (r == 0) => d,
        (d, _) => d + 1,
    }
}

impl fmt::Debug for TransportWrapper {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(
            f,
            "TransportWrapper {{ is initiator: {} }}",
            self.state.is_initiator()
        )
    }
}
