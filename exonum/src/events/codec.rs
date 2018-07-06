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

use failure::Error;

use events::noise::wrapper::{NoiseWrapper, NOISE_HEADER_LENGTH};
use messages::{SignedMessage, UncheckedBuffer};

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
    type Item = UncheckedBuffer;
    type Error = Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>, Error> {
        const U32_LENGTH: usize = 4;

        // Read size
        if buf.len() < U32_LENGTH {
            return Ok(None);
        }

        let len = LittleEndian::read_u32(buf) as usize;

        // To fix some weird `decode()` behavior https://github.com/carllerche/bytes/issues/104
        if buf.len() < len + NOISE_HEADER_LENGTH {
            return Ok(None);
        }

        let buf = self.session.decrypt_msg(len, buf);

        // Read message
        let data = buf.to_vec();
        Ok(Some(UncheckedBuffer::new(data)))
    }
}

impl Encoder for MessagesCodec {
    type Item = SignedMessage;
    type Error = Error;

    fn encode(&mut self, msg: Self::Item, buf: &mut BytesMut) -> Result<(), Error> {
        let data = msg.to_vec();
        self.session.encrypt_msg(&data, buf);
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::MessagesCodec;

    use bytes::BytesMut;
    use crypto::{gen_keypair_from_seed, PublicKey, SecretKey, Seed};
    use events::noise::wrapper::NoiseWrapper;
    use events::noise::HandshakeParams;
    use messages::{Message, SignedMessage};
    use tokio_io::codec::{Decoder, Encoder};

    pub fn raw_message(id: u16, tx: Vec<u8>, keypair: (PublicKey, &SecretKey)) -> SignedMessage {
        Message::create_raw_tx(tx, id, keypair).into_parts().1
    }

    #[test]
    fn decode_message_small_size_in_header() {
        let data = vec![0_u8, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let mut bytes: BytesMut = BytesMut::new();
        let (p, k) = gen_keypair_from_seed(&Seed::new([1; 32]));

        let signed = raw_message(0, data, (p, &k));
        let source = signed.to_vec();
        let (ref mut responder, ref mut initiator) = create_encrypted_codecs();
        initiator.encode(signed, &mut bytes).unwrap();

        assert_eq!(
            responder.decode(&mut bytes).unwrap().unwrap().get_vec(),
            &source
        );
    }

    fn create_encrypted_codecs() -> (MessagesCodec, MessagesCodec) {
        let (public_key, secret_key) = gen_keypair_from_seed(&Seed::new([1; 32]));
        let mut params = HandshakeParams::new(public_key, secret_key, 1024);
        params.set_remote_key(public_key);

        let mut initiator = NoiseWrapper::initiator(&params).session;
        let mut responder = NoiseWrapper::responder(&params).session;

        let mut buffer_msg = vec![0u8; 1024];
        let mut buffer_out = [0u8; 1024];

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
