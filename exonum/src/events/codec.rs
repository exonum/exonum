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

use byteorder::{ByteOrder, LittleEndian};
use bytes::BytesMut;
use std::mem;
use tokio_io::codec::{Decoder, Encoder};

use crate::{
    events::noise::{TransportWrapper, HEADER_LENGTH as NOISE_HEADER_LENGTH},
    merkledb::BinaryValue,
    messages::{SignedMessage, SIGNED_MESSAGE_MIN_SIZE},
};
#[derive(Debug)]
pub struct MessagesCodec {
    /// Maximum message length (in bytes), gets populated from `ConsensusConfig`.
    max_message_len: u32,
    /// Noise session to encrypt/decrypt messages.
    session: TransportWrapper,
}

impl MessagesCodec {
    pub fn new(max_message_len: u32, session: TransportWrapper) -> Self {
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

        let buf = self.session.decrypt_msg(len, buf)?;

        if buf.len() > self.max_message_len as usize {
            bail!(
                "Received message is too long: received_len = {}, allowed_len = {}",
                buf.len(),
                self.max_message_len
            )
        }

        if buf.len() <= SIGNED_MESSAGE_MIN_SIZE {
            bail!(
                "Received malicious message with wrong length: received_len = {}, min_len = {}",
                buf.len(),
                SIGNED_MESSAGE_MIN_SIZE
            )
        }

        Ok(Some(buf.to_vec()))
    }
}

impl Encoder for MessagesCodec {
    type Item = SignedMessage;
    type Error = failure::Error;

    fn encode(&mut self, msg: Self::Item, buf: &mut BytesMut) -> Result<(), Self::Error> {
        self.session.encrypt_msg(&msg.into_bytes(), buf)?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use bytes::BytesMut;
    use exonum_merkledb::BinaryValue;
    use tokio_io::codec::{Decoder, Encoder};

    use crate::{
        crypto::{gen_keypair, Hash},
        events::noise::{HandshakeParams, NoiseWrapper, TransportWrapper},
        helpers::Height,
        messages::{Status, Verified, SIGNED_MESSAGE_MIN_SIZE},
    };

    use super::MessagesCodec;

    fn get_decoded_message(data: &[u8]) -> Result<Option<Vec<u8>>, failure::Error> {
        let (ref mut responder, ref mut initiator) = create_encrypted_codecs();

        let mut bytes: BytesMut = BytesMut::new();
        initiator.session.encrypt_msg(data, &mut bytes).unwrap();

        responder.decode(&mut bytes)
    }

    fn create_encrypted_codecs() -> (MessagesCodec, MessagesCodec) {
        let params = HandshakeParams::with_default_params();

        let mut initiator = NoiseWrapper::initiator(&params).state;
        let mut responder = NoiseWrapper::responder(&params).state;

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

        let responder = TransportWrapper {
            state: responder.into_transport_mode().unwrap(),
        };
        let initiator = TransportWrapper {
            state: initiator.into_transport_mode().unwrap(),
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

    #[test]
    fn decode_message_valid_header_size() {
        let data = vec![0; SIGNED_MESSAGE_MIN_SIZE + 1];

        match get_decoded_message(&data) {
            Ok(Some(ref message)) if *message == &data[..] => {}
            _ => panic!("Wrong input"),
        };
    }

    #[test]
    #[should_panic(expected = "Received malicious message with wrong length")]
    fn decode_message_small_length() {
        let data = vec![0; SIGNED_MESSAGE_MIN_SIZE - 10];

        get_decoded_message(&data).unwrap();
    }

    #[test]
    fn decode_message_eof() {
        let (ref mut responder, ref mut initiator) = create_encrypted_codecs();

        let raw = {
            let (pk, sk) = gen_keypair();
            let msg = Verified::from_value(Status::new(Height(0), Hash::zero(), 0), pk, &sk);
            msg.into_raw()
        };
        let data = raw.to_bytes();

        let mut bytes: BytesMut = BytesMut::new();
        initiator.encode(raw.clone(), &mut bytes).unwrap();
        initiator.encode(raw, &mut bytes).unwrap();

        match responder.decode_eof(&mut bytes.clone()) {
            Ok(Some(ref message)) if *message == &data[..] => {}
            _ => panic!("Wrong input"),
        };

        // Emulate EOF behavior.
        bytes.truncate(1);
        assert!(responder.decode(&mut bytes).unwrap().is_none());

        bytes.clear();
        assert!(responder.decode_eof(&mut bytes).unwrap().is_none());
    }
}
