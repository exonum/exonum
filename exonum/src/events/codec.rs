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

use bytes::BytesMut;
use byteorder::{ByteOrder, LittleEndian};
use tokio_io::codec::{Decoder, Encoder};
use failure::Error;
use messages::{UncheckedBuffer, SignedMessage};
#[derive(Debug)]
pub struct MessagesCodec {
    /// Maximum message length (in bytes), gets populated from `ConsensusConfig`.
    max_message_len: u32,
}

impl MessagesCodec {
    pub fn new(max_message_len: u32) -> MessagesCodec {
        MessagesCodec { max_message_len }
    }
}

impl Decoder for MessagesCodec {
    type Item = UncheckedBuffer;
    type Error = Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        unimplemented!();
        // Read header
        /*if buf.len() < HEADER_LENGTH {
            return Ok(None);
        }

        if buf[0] != 0 {
            return Err(other_error("Message first byte must be set to 0"));
        }

        // Check payload len
        let total_len = LittleEndian::read_u32(&buf) as usize;

        if total_len as u32 > self.max_message_len {
            bail!(
                "Received message is too long: {}, maximum allowed length is {} bytes",
                total_len,
                self.max_message_len,
            );
        }

        if total_len < HEADER_LENGTH {
            bail!(
                "Received malicious message with insufficient \
                size in header: {}, expected header size {}",
                total_len,
                HEADER_LENGTH
            );
        }

        // Read message
        if buf.len() + 4 >= total_len {
            buf.advance(4); //ignore total_len
            let data = buf.split_to(total_len).to_vec();
            let raw = UncheckedBuffer::new(data);
            return Ok(Some(raw));
        }
        Ok(None) */
    }
}

impl Encoder for MessagesCodec {
    type Item = SignedMessage;
    type Error = Error;

    fn encode(&mut self, msg: Self::Item, buf: &mut BytesMut) -> Result<(), Self::Error> {
        /*let buffer = msg.into_buffer()?;
        buf.write_u32::<LittleEndian>(buffer.len() as u32);
        buf.extend_from_slice(&buffer);
        */
        unimplemented!();
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::MessagesCodec;

    use messages::{MessageBuffer, RawMessage};
    use bytes::BytesMut;
    use tokio_io::codec::Decoder;

    #[test]
    fn decode_message_valid_header_size() {
        let data = vec![0u8, 0, 0, 0, 0, 0, 10, 0, 0, 0];
        let mut bytes: BytesMut = data.as_slice().into();
        let mut codec = MessagesCodec {
            max_message_len: 10000,
        };
        match codec.decode(&mut bytes) {
            Ok(Some(ref r)) if r == &RawMessage::new(MessageBuffer::from_vec(data)) => {}
            _ => panic!("Wrong input"),
        };
    }

    #[test]
    fn decode_message_small_size_in_header() {
        let data = vec![0u8, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let mut bytes: BytesMut = data.as_slice().into();
        let mut codec = MessagesCodec {
            max_message_len: 10000,
        };
        assert!(codec.decode(&mut bytes).is_err());
    }

    #[test]
    fn decode_message_zero_byte() {
        let data = vec![1u8, 0, 0, 0, 0, 0, 10, 0, 0, 0];
        let mut bytes: BytesMut = data.as_slice().into();
        let mut codec = MessagesCodec {
            max_message_len: 10000,
        };
        assert!(codec.decode(&mut bytes).is_err());
    }
}
