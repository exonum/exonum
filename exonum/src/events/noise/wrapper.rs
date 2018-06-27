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
use bytes::BytesMut;
use failure;
use snow::{NoiseBuilder, Session};

use std::{fmt::{self, Error, Formatter},
          io};

use events::noise::sodium_resolver::SodiumResolver;
use events::noise::HandshakeParams;

pub const NOISE_MAX_MESSAGE_LENGTH: usize = 65_535;
pub const TAG_LENGTH: usize = 16;
pub const NOISE_HEADER_LENGTH: usize = 4;
pub const HANDSHAKE_HEADER_LENGTH: usize = 1;
pub const NOISE_MAX_HANDSHAKE_MESSAGE_LENGTH: usize = 255;
pub const NOISE_MIN_HANDSHAKE_MESSAGE_LENGTH: usize = 32;

// We choose XX pattern since it provides mutual authentication and
// transmission of static public keys.
// See: https://noiseprotocol.org/noise.html#interactive-patterns
static PARAMS: &str = "Noise_XK_25519_ChaChaPoly_SHA256";

/// Wrapper around noise session to provide latter convenient interface.
pub struct NoiseWrapper {
    pub session: Session,
}

impl NoiseWrapper {
    pub fn initiator(params: &HandshakeParams) -> Self {
        if let Some(ref remote_key) = params.remote_key {
            let builder: NoiseBuilder = Self::noise_builder()
                .local_private_key(params.secret_key.as_ref())
                .remote_public_key(remote_key.as_ref());
            let session = builder
                .build_initiator()
                .expect("Noise session initiator failed to initialize");
            return NoiseWrapper { session };
        } else {
            panic!("Remote public key is not specified")
        }
    }

    pub fn responder(params: &HandshakeParams) -> Self {
        let builder: NoiseBuilder = Self::noise_builder();

        let session = builder
            .local_private_key(params.secret_key.as_ref())
            .build_responder()
            .expect("Noise session responder failed to initialize");

        NoiseWrapper { session }
    }

    pub fn read_handshake_msg(&mut self, input: &[u8]) -> Result<Vec<u8>, NoiseError> {
        if input.len() < NOISE_MIN_HANDSHAKE_MESSAGE_LENGTH
            || input.len() > NOISE_MAX_MESSAGE_LENGTH
        {
            return Err(NoiseError::WrongMessageLength(input.len()));
        }

        self.read(input, NOISE_MAX_MESSAGE_LENGTH)
    }

    pub fn write_handshake_msg(&mut self) -> Result<Vec<u8>, NoiseError> {
        // Payload in handshake messages can be empty.
        self.write(&[])
    }

    pub fn into_transport_mode(self) -> Result<Self, NoiseError> {
        // Transition into transport mode after handshake is finished.
        let session = self.session.into_transport_mode()?;
        Ok(NoiseWrapper { session })
    }

    /// Decrypts `msg` using Noise session.
    ///
    /// Decryption consists of the following steps:
    /// 1. Message splits to packets of length smaller or equal to 65_535 bytes.
    /// 2. Then each packet is decrypted by selected noise algorithm.
    /// 3. Append all decrypted packets to `decoded_message`.
    pub fn decrypt_msg(&mut self, len: usize, buf: &mut BytesMut) -> Result<BytesMut, io::Error> {
        let data = buf.split_to(len + NOISE_HEADER_LENGTH).to_vec();
        let data = &data[NOISE_HEADER_LENGTH..];
        let mut decoded_message = vec![0u8; 0];

        data.chunks(NOISE_MAX_MESSAGE_LENGTH).for_each(|msg| {
            let len_to_read = if msg.len() == NOISE_MAX_MESSAGE_LENGTH {
                msg.len() - TAG_LENGTH
            } else {
                msg.len()
            };

            let read_to = self.read(msg, len_to_read).unwrap();
            decoded_message.extend_from_slice(&read_to);
        });

        Ok(BytesMut::from(decoded_message))
    }

    /// Encrypts `msg` using Noise session
    ///
    /// Encryption consists of the following steps:
    /// 1. Message splits to packets of length smaller or equal to 65_535 bytes.
    /// 2. Then each packet is encrypted by selected noise algorithm.
    /// 3. Result message: first 4 bytes is message length(`len').
    /// 4. Append all encrypted packets in corresponding order.
    /// 5. Write result message to `buf`
    pub fn encrypt_msg(&mut self, msg: &[u8], buf: &mut BytesMut) -> Result<Option<()>, io::Error> {
        let mut len = 0usize;
        let mut encoded_message = vec![0u8; 0];

        msg.chunks(NOISE_MAX_MESSAGE_LENGTH - TAG_LENGTH)
            .for_each(|msg| {
                let written = self.write(msg).unwrap();
                encoded_message.extend_from_slice(&written);
                len += written.len();
            });

        let mut msg_len_buf = vec![0u8; NOISE_HEADER_LENGTH];

        LittleEndian::write_u32(&mut msg_len_buf, len as u32);
        let encoded_message = &encoded_message[0..len];
        msg_len_buf.extend_from_slice(encoded_message);
        buf.extend_from_slice(&msg_len_buf);
        Ok(None)
    }

    fn read(&mut self, input: &[u8], len: usize) -> Result<Vec<u8>, NoiseError> {
        let mut buf = vec![0u8; len];
        self.session.read_message(input, &mut buf)?;
        Ok(buf)
    }

    fn write(&mut self, msg: &[u8]) -> Result<Vec<u8>, NoiseError> {
        let mut buf = vec![0u8; NOISE_MAX_MESSAGE_LENGTH];
        let len = self.session.write_message(msg, &mut buf)?;
        Ok(buf[..len].to_vec())
    }

    fn noise_builder<'a>() -> NoiseBuilder<'a> {
        NoiseBuilder::with_resolver(PARAMS.parse().unwrap(), Box::new(SodiumResolver::new()))
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

#[derive(Fail, Debug, Clone)]
pub enum NoiseError {
    #[fail(display = "Wrong handshake message length {}", _0)]
    WrongMessageLength(usize),

    #[fail(display = "{}", _0)]
    Other(String),
}

impl From<NoiseError> for io::Error {
    fn from(e: NoiseError) -> Self {
        let message = match e {
            NoiseError::Other(message) => message,
            _ => format!("{:?}", e),
        };

        io::Error::new(io::ErrorKind::Other, message)
    }
}

impl From<failure::Error> for NoiseError {
    fn from(e: failure::Error) -> Self {
        NoiseError::Other(format!("{:?}", e))
    }
}
