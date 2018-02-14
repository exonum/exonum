// Copyright 2017 The Exonum Team
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

use std::io;

use bytes::BytesMut;
use byteorder::{LittleEndian, ByteOrder};
use tokio_io::codec::{Decoder, Encoder};

use messages::{HEADER_LENGTH, MessageBuffer, RawMessage};
use super::error::other_error;

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
    type Item = RawMessage;
    type Error = io::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>, io::Error> {
        // Read header
        if buf.len() < HEADER_LENGTH {
            return Ok(None);
        }
        // Check payload len
        let total_len = LittleEndian::read_u32(&buf[6..10]) as usize;

        if total_len as u32 > self.max_message_len {
            return Err(other_error(format!(
                "Received message is too long: {}, maximum allowed length is {} bytes",
                total_len,
                self.max_message_len,
            )));
        }

        if total_len < HEADER_LENGTH {
            return Err(other_error(format!(
                "Received malicious message with insufficient \
                size in header: {}, expected header size {}",
                total_len,
                HEADER_LENGTH
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
        buf.extend_from_slice(msg.as_ref());
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
        let mut codec = MessagesCodec { max_message_len: 10000 };
        match codec.decode(&mut bytes) {
            Ok(Some(ref r)) if r == &RawMessage::new(MessageBuffer::from_vec(data)) => {}
            _ => panic!("Wrong input"),
        };
    }

    #[test]
    fn decode_message_small_size_in_header() {
        let data = vec![0u8, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let mut bytes: BytesMut = data.as_slice().into();
        let mut codec = MessagesCodec { max_message_len: 10000 };
        assert!(codec.decode(&mut bytes).is_err());
    }
}
