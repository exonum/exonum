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
use std::mem;
use tokio_io::codec::{Decoder, Encoder};

use events::noise::{NoiseWrapper, HEADER_LENGTH as NOISE_HEADER_LENGTH};
use messages::SignedMessage;

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
    type Item = Vec<u8>;
    type Error = failure::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        // Framing level
        if buf.len() < mem::size_of::<u32>() {
            return Ok(None);
        }

        let len = LittleEndian::read_u32(buf) as usize;

        if buf.len() < NOISE_HEADER_LENGTH + len {
            return Ok(None);
        }

        let mut buf = self.session.decrypt_msg(len, buf)?;

        if buf.len() > NOISE_HEADER_LENGTH + self.max_message_len as usize {
            bail!(
                "Message too big received_len = {}, allowd_len = {}",
                buf.len() - NOISE_HEADER_LENGTH,
                self.max_message_len
            )
        }

        Ok(Some(buf.to_vec()))
    }
}

impl Encoder for MessagesCodec {
    type Item = SignedMessage;
    type Error = failure::Error;

    fn encode(&mut self, msg: Self::Item, buf: &mut BytesMut) -> Result<(), Self::Error> {
        self.session.encrypt_msg(msg.raw(), buf)?;
        Ok(())
    }
}

//#[cfg(test)]
#[cfg(test2)]
mod test {
    use bytes::BytesMut;
    use failure;
    use tokio_io::codec::{Decoder, Encoder};

    use super::MessagesCodec;
    use crypto::{gen_keypair_from_seed, Seed, SEED_LENGTH};
    use events::noise::{HandshakeParams, NoiseWrapper};
    use node::state::SharedConnectList;

    #[test]
    fn decode_message_valid_header_size() {
        let data = vec![0_u8, 0, 0, 0, 0, 0, 10, 0, 0, 0];

        match get_decoded_message(&data) {
            Ok(Some(ref message)) if *message.as_ref() == data[..] => {}
            _ => panic!("Wrong input"),
        };
    }

    #[test]
    fn decode_message_small_size_in_header() {
        let data = vec![0_u8, 0, 0, 0, 0, 0, 0, 0, 0, 0];

        assert!(get_decoded_message(&data).is_err());
    }

    #[test]
    fn decode_message_zero_byte() {
        let data = vec![1u8, 0, 0, 0, 0, 0, 10, 0, 0, 0];

        assert!(get_decoded_message(&data).is_err());
    }

    #[test]
    #[should_panic(expected = "Received malicious message with wrong total_len")]
    fn decode_message_small_length() {
        let data = vec![0u8, 0, 0, 0, 0, 0, 11, 0, 0, 0, 0, 0];

        get_decoded_message(&data).unwrap();
    }

    #[test]
    #[should_panic(expected = "Received malicious message with wrong total_len")]
    fn decode_message_big_length() {
        let data = vec![0u8, 0, 0, 0, 0, 0, 11, 0, 0, 0];

        get_decoded_message(&data).unwrap();
    }

    #[test]
    fn decode_message_eof() {
        let (ref mut responder, ref mut initiator) = create_encrypted_codecs();

        let data = vec![0u8, 0, 0, 0, 0, 0, 10, 0, 0, 0];
        let raw = RawMessage::new(MessageBuffer::from_vec(data.clone()));

        let mut bytes: BytesMut = BytesMut::new();
        initiator.encode(raw.clone(), &mut bytes).unwrap();
        initiator.encode(raw, &mut bytes).unwrap();

        match responder.decode_eof(&mut bytes.clone()) {
            Ok(Some(ref message)) if *message.as_ref() == data[..] => {}
            _ => panic!("Wrong input"),
        };

        // Emulate EOF behavior.
        bytes.truncate(1);
        assert!(responder.decode_eof(&mut bytes).unwrap().is_none());

        bytes.clear();
        assert!(responder.decode_eof(&mut bytes).unwrap().is_none());
    }

    fn get_decoded_message(data: &[u8]) -> Result<Option<RawMessage>, failure::Error> {
        let (ref mut responder, ref mut initiator) = create_encrypted_codecs();
        let raw = RawMessage::new(MessageBuffer::from_vec(data.to_vec()));

        let mut bytes: BytesMut = BytesMut::new();
        initiator.encode(raw, &mut bytes).unwrap();

        responder.decode(&mut bytes)
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
