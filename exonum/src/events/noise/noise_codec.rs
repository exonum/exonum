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

use std::io;
use messages::RawMessage;
use messages::MessageBuffer;
use super::wrapper::{NoiseWrapper, HANDSHAKE_HEADER_LEN, HEADER_LEN, NOISE_MAX_MESSAGE_LEN, TAGLEN};
use events::error::other_error;

#[allow(missing_debug_implementations)]
#[allow(dead_code)]
pub struct NoiseCodec {
    session: NoiseWrapper,
    max_message_len: u32,
}

impl NoiseCodec {
    #[allow(dead_code)]
    pub fn new(session: NoiseWrapper, max_message_len: u32) -> Self {
        NoiseCodec {
            session,
            max_message_len,
        }
    }
}

impl Decoder for NoiseCodec {
    type Item = RawMessage;
    type Error = io::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>, io::Error> {
        if buf.len() < HANDSHAKE_HEADER_LEN {
            return Ok(None);
        };

        let len = LittleEndian::read_u32(buf) as usize;

        if len > buf.len() {
            return Ok(None);
        }

        let len_diff = (buf.len() / NOISE_MAX_MESSAGE_LEN + 1) * TAGLEN;

        if len as u32 > self.max_message_len + len_diff as u32 {
            return Err(other_error(format!(
                "Received message is too long: {}, maximum allowed length is {} bytes",
                len, self.max_message_len,
            )));
        }

        let data = buf.split_to(len + HEADER_LEN).to_vec();
        let data = &data[HEADER_LEN..];
        let mut decoded_message = vec![0u8; 0];

        data.chunks(NOISE_MAX_MESSAGE_LEN).for_each(|msg| {
            let len_to_read = if msg.len() == NOISE_MAX_MESSAGE_LEN {
                msg.len() - TAGLEN
            } else {
                msg.len()
            };

            let (_, read_to) = self.session.read(msg, len_to_read).unwrap();
            decoded_message.extend_from_slice(&read_to);
        });

        let total_len = LittleEndian::read_u32(&decoded_message[6..10]) as usize;
        let decoded_message = decoded_message.split_at(total_len);

        let raw = RawMessage::new(MessageBuffer::from_vec(Vec::from(decoded_message.0)));
        Ok(Some(raw))
    }
}

impl Encoder for NoiseCodec {
    type Item = RawMessage;
    type Error = io::Error;

    fn encode(&mut self, msg: Self::Item, buf: &mut BytesMut) -> io::Result<()> {
        let mut len = 0usize;
        let mut encoded_message = vec![0u8; 0];

        msg.as_ref()
            .chunks(NOISE_MAX_MESSAGE_LEN - TAGLEN)
            .for_each(|msg| {
                let (written_bytes, written) = self.session.write(msg).unwrap();
                encoded_message.extend_from_slice(&written);
                len += written_bytes;
            });

        let mut msg_len_buf = vec![0u8; HEADER_LEN];

        LittleEndian::write_u32(&mut msg_len_buf, len as u32);
        let encoded_message = &encoded_message[0..len];
        msg_len_buf.extend_from_slice(encoded_message);
        buf.extend_from_slice(&msg_len_buf);
        Ok(())
    }
}
