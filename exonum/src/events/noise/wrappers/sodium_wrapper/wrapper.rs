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

// Workaround for `failure` see https://github.com/rust-lang-nursery/failure/issues/223 and
// ECR-1771 for the details.
#![allow(bare_trait_objects)]

use byteorder::{ByteOrder, LittleEndian};
use bytes::BytesMut;
use failure;
use snow::{Builder, Session};

use std::fmt::{self, Error, Formatter};

use super::{handshake::HandshakeParams, resolver::SodiumResolver};
use events::noise::{error::NoiseError, HEADER_LENGTH, MAX_MESSAGE_LENGTH, TAG_LENGTH};

pub const HANDSHAKE_HEADER_LENGTH: usize = 1;
pub const MAX_HANDSHAKE_MESSAGE_LENGTH: usize = 255;
pub const MIN_HANDSHAKE_MESSAGE_LENGTH: usize = 32;

// We choose XK pattern since it provides mutual authentication,
// transmission of static public keys and requires pre-defined remote public
// key to establish connection.
// See: https://noiseprotocol.org/noise.html#interactive-patterns
static PARAMS: &str = "Noise_XK_25519_ChaChaPoly_SHA256";

/// Wrapper around noise session to provide latter convenient interface.
pub struct NoiseWrapper {
    pub session: Session,
}

impl NoiseWrapper {
    pub fn initiator(params: &HandshakeParams) -> Self {
        if let Some(ref remote_key) = params.remote_key {
            let builder: Builder = Self::noise_builder()
                .local_private_key(params.secret_key.as_ref())
                .remote_public_key(remote_key.as_ref());
            let session = builder
                .build_initiator()
                .expect("Noise session initiator failed to initialize");
            return Self { session };
        } else {
            panic!("Remote public key is not specified")
        }
    }

    pub fn responder(params: &HandshakeParams) -> Self {
        let builder: Builder = Self::noise_builder();

        let session = builder
            .local_private_key(params.secret_key.as_ref())
            .build_responder()
            .expect("Noise session responder failed to initialize");

        Self { session }
    }

    pub fn read_handshake_msg(&mut self, input: &[u8]) -> Result<Vec<u8>, NoiseError> {
        if input.len() < MIN_HANDSHAKE_MESSAGE_LENGTH || input.len() > MAX_MESSAGE_LENGTH {
            return Err(NoiseError::WrongMessageLength(input.len()));
        }

        self.read(input, MAX_MESSAGE_LENGTH)
    }

    pub fn write_handshake_msg(&mut self) -> Result<Vec<u8>, NoiseError> {
        // Payload in handshake messages can be empty.
        self.write(&[])
    }

    pub fn into_transport_mode(self) -> Result<Self, NoiseError> {
        // Transition into transport mode after handshake is finished.
        let session = self.session.into_transport_mode()?;
        Ok(Self { session })
    }

    /// Decrypts `msg` using Noise session.
    ///
    /// Decryption consists of the following steps:
    /// 1. Message splits to packets of length smaller or equal to 65_535 bytes.
    /// 2. Then each packet is decrypted by selected noise algorithm.
    /// 3. Append all decrypted packets to `decoded_message`.
    pub fn decrypt_msg(
        &mut self,
        len: usize,
        buf: &mut BytesMut,
    ) -> Result<BytesMut, failure::Error> {
        let data = buf.split_to(len + HEADER_LENGTH).to_vec();
        let data = &data[HEADER_LENGTH..];

        let len = self.decrypted_msg_len(data.len());
        let mut decrypted_message = Vec::with_capacity(len);

        for msg in data.chunks(MAX_MESSAGE_LENGTH) {
            let len_to_read = if msg.len() == MAX_MESSAGE_LENGTH {
                msg.len() - TAG_LENGTH
            } else {
                msg.len()
            };

            let read_to = self.read(msg, len_to_read)?;
            decrypted_message.extend_from_slice(&read_to);
        }

        debug_assert_eq!(len, decrypted_message.len());
        Ok(BytesMut::from(decrypted_message))
    }

    /// Encrypts `msg` using Noise session
    ///
    /// Encryption consists of the following steps:
    /// 1. Message splits to packets of length smaller or equal to 65_535 bytes.
    /// 2. Then each packet is encrypted by selected noise algorithm.
    /// 3. Result message: first 4 bytes is message length(`len').
    /// 4. Append all encrypted packets in corresponding order.
    /// 5. Write result message to `buf`
    pub fn encrypt_msg(
        &mut self,
        msg: &[u8],
        buf: &mut BytesMut,
    ) -> Result<Option<()>, failure::Error> {
        let len = self.encrypted_msg_len(msg.len());
        let mut encrypted_message = Vec::with_capacity(len);

        for msg in msg.chunks(MAX_MESSAGE_LENGTH - TAG_LENGTH) {
            let written = self.write(msg)?;
            encrypted_message.extend_from_slice(&written);
        }

        let mut msg_len_buf = vec![0_u8; HEADER_LENGTH];

        LittleEndian::write_u32(&mut msg_len_buf, len as u32);
        let encoded_message = &encrypted_message[0..len];
        msg_len_buf.extend_from_slice(encoded_message);
        buf.extend_from_slice(&msg_len_buf);

        debug_assert_eq!(len, encoded_message.len());
        Ok(None)
    }

    fn read(&mut self, input: &[u8], len: usize) -> Result<Vec<u8>, NoiseError> {
        let mut buf = vec![0_u8; len];
        self.session.read_message(input, &mut buf)?;
        Ok(buf)
    }

    fn write(&mut self, msg: &[u8]) -> Result<Vec<u8>, NoiseError> {
        let mut buf = vec![0_u8; MAX_MESSAGE_LENGTH];
        let len = self.session.write_message(msg, &mut buf)?;
        buf.truncate(len);
        Ok(buf)
    }

    // Each message consists of the payload and 16 bytes(`TAG_LENGTH`)
    // of AEAD authentication data. Therefore to calculate an actual message
    // length we need to subtract `TAG_LENGTH` multiplied by messages count
    // from `data.len()`.
    fn decrypted_msg_len(&self, raw_message_len: usize) -> usize {
        raw_message_len - TAG_LENGTH * (raw_message_len / MAX_MESSAGE_LENGTH)
    }

    // In case of encryption we need to add `TAG_LENGTH` multiplied by messages count to
    // calculate actual message length.
    fn encrypted_msg_len(&self, raw_message_len: usize) -> usize {
        raw_message_len + TAG_LENGTH * ((raw_message_len / MAX_MESSAGE_LENGTH) + 1)
    }

    fn noise_builder<'a>() -> Builder<'a> {
        Builder::with_resolver(PARAMS.parse().unwrap(), Box::new(SodiumResolver::new()))
    }
}

impl fmt::Debug for NoiseWrapper {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        write!(
            f,
            "NoiseWrapper {{ handshake finished: {} }}",
            self.session.is_handshake_finished()
        )
    }
}
