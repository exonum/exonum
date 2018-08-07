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
use tokio_io::codec::{Decoder, Encoder};

use std::io;

use super::error::other_error;
use events::noise::{NoiseWrapper, HEADER_LENGTH as NOISE_HEADER_LENGTH};
use messages::{MessageBuffer, RawMessage, HEADER_LENGTH};

#[derive(Debug)]
pub struct MessagesCodec {
    /// Maximum message length (in bytes), gets populated from `ConsensusConfig`.
    max_message_len: u32,
    /// Noise session to encrypt/decrypt messages.
    session: NoiseWrapper,
}

impl MessagesCodec {
    pub fn new(max_message_len: u32, session: NoiseWrapper) -> Self {
        Self {
            max_message_len,
            session,
        }
    }
}

impl Decoder for MessagesCodec {
    type Item = RawMessage;
    type Error = io::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>, io::Error> {
        // Read header
        if buf.len() < HEADER_LENGTH {
            return Ok(None);
        }

        let len = LittleEndian::read_u32(buf) as usize;

        // To fix some weird `decode()` behavior https://github.com/carllerche/bytes/issues/104
        if buf.len() < len + NOISE_HEADER_LENGTH {
            return Ok(None);
        }

        let mut buf = self.session.decrypt_msg(len, buf)?;

        if buf[0] != 0 {
            return Err(other_error("Message first byte must be set to 0"));
        }

        // Check payload len
        let total_len = LittleEndian::read_u32(&buf[6..10]) as usize;

        if total_len as u32 > self.max_message_len {
            return Err(other_error(format!(
                "Received message is too long: {}, maximum allowed length is {} bytes",
                total_len, self.max_message_len,
            )));
        }

        if total_len < HEADER_LENGTH {
            return Err(other_error(format!(
                "Received malicious message with insufficient \
                 size in header: {}, expected header size {}",
                total_len, HEADER_LENGTH
            )));
        }

        // Read message
        if buf.len() >= total_len {
            let data = buf.split_to(total_len).to_vec();
            let raw = RawMessage::new(MessageBuffer::from_vec(data));
            return Ok(Some(raw));
        }
        Ok(None)
    }
}

impl Encoder for MessagesCodec {
    type Item = RawMessage;
    type Error = io::Error;

    fn encode(&mut self, msg: Self::Item, buf: &mut BytesMut) -> io::Result<()> {
        self.session.encrypt_msg(msg.as_ref(), buf)?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::MessagesCodec;

    use bytes::BytesMut;
    use crypto::{gen_keypair_from_seed, Seed, SEED_LENGTH};
    use events::noise::{HandshakeParams, NoiseWrapper};
    use messages::{MessageBuffer, RawMessage};
    use node::state::SharedConnectList;
    use tokio_io::codec::{Decoder, Encoder};

    #[test]
    fn decode_message_valid_header_size() {
        let data = vec![0_u8, 0, 0, 0, 0, 0, 10, 0, 0, 0];
        let mut bytes: BytesMut = BytesMut::new();
        let (ref mut responder, ref mut initiator) = create_encrypted_codecs();
        let raw = RawMessage::new(MessageBuffer::from_vec(data.clone()));
        initiator.encode(raw, &mut bytes).unwrap();

        match responder.decode(&mut bytes) {
            Ok(Some(ref r)) if r == &RawMessage::new(MessageBuffer::from_vec(data)) => {}
            _ => panic!("Wrong input"),
        };
    }

    #[test]
    fn decode_message_small_size_in_header() {
        let data = vec![0_u8, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let mut bytes: BytesMut = BytesMut::new();
        let (ref mut responder, ref mut initiator) = create_encrypted_codecs();
        let raw = RawMessage::new(MessageBuffer::from_vec(data));
        initiator.encode(raw, &mut bytes).unwrap();

        assert!(responder.decode(&mut bytes).is_err());
    }

    #[test]
    fn decode_message_zero_byte() {
        let data = vec![1u8, 0, 0, 0, 0, 0, 10, 0, 0, 0];
        let mut bytes: BytesMut = BytesMut::new();
        let (ref mut responder, ref mut initiator) = create_encrypted_codecs();
        let raw = RawMessage::new(MessageBuffer::from_vec(data));

        initiator.encode(raw, &mut bytes).unwrap();
        assert!(responder.decode(&mut bytes).is_err());
    }

    fn create_encrypted_codecs() -> (MessagesCodec, MessagesCodec) {
        let (public_key, secret_key) = gen_keypair_from_seed(&Seed::new([1; SEED_LENGTH]));
        let mut params =
            HandshakeParams::new(public_key, secret_key, SharedConnectList::default(), 1024);
        params.set_remote_key(public_key);

        let mut initiator = NoiseWrapper::initiator(&params).session;
        let mut responder = NoiseWrapper::responder(&params).session;

        let mut buffer_msg = vec![0_u8; 1024];
        let mut buffer_out = [0_u8; 1024];

        // Simple handshake for testing.
        let len = initiator
            .write_message(&[0_u8; 0], &mut buffer_msg)
            .unwrap();
        responder
            .read_message(&buffer_msg[..len], &mut buffer_out)
            .unwrap();
        let len = responder
            .write_message(&[0_u8; 0], &mut buffer_msg)
            .unwrap();
        initiator
            .read_message(&buffer_msg[..len], &mut buffer_out)
            .unwrap();
        let len = initiator
            .write_message(&[0_u8; 0], &mut buffer_msg)
            .unwrap();
        responder
            .read_message(&buffer_msg[..len], &mut buffer_out)
            .unwrap();

        let responder = NoiseWrapper {
            session: responder.into_transport_mode().unwrap(),
        };
        let initiator = NoiseWrapper {
            session: initiator.into_transport_mode().unwrap(),
        };

        let responder_codec = MessagesCodec {
            max_message_len: 10000,
            session: initiator,
        };

        let initiator_codec = MessagesCodec {
            max_message_len: 10000,
            session: responder,
        };

        (responder_codec, initiator_codec)
    }
}
