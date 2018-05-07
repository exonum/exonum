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
use snow::Session;

use std::io;
use messages::RawMessage;
use messages::MessageBuffer;

#[allow(missing_debug_implementations)]
pub struct NoiseCodec {
    session: Session,
}

impl NoiseCodec {
    pub fn new(session: Session) -> Self {
        NoiseCodec { session }
    }
}

impl Decoder for NoiseCodec {
    type Item = RawMessage;
    type Error = io::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>, io::Error> {
        if buf.len() < 2 {
            return Ok(None);
        };

        let len = LittleEndian::read_u32(buf) as usize;

        if len > buf.len() {
            return Ok(None);
        }

        let data = buf.split_to(len + 4).to_vec();
        let data = &data[4..];
        let mut readed_data = vec![0u8; 0];
        let mut readed_len = 0usize;

        data.chunks(65535).for_each(|chunk| {
            let mut read_to = vec![0u8; chunk.len()];
            readed_len += self.session.read_message(chunk, &mut read_to).unwrap();
            readed_data.extend_from_slice(&read_to);
        });

        let total_len = LittleEndian::read_u32(&readed_data[6..10]) as usize;

        let data = readed_data.split_at(total_len);
        let raw = RawMessage::new(MessageBuffer::from_vec(Vec::from(data.0)));
        Ok(Some(raw))
    }
}

impl Encoder for NoiseCodec {
    type Item = RawMessage;
    type Error = io::Error;

    fn encode(&mut self, msg: Self::Item, buf: &mut BytesMut) -> io::Result<()> {
        let mut len = 0usize;

        let mut write_to_buf = vec![0u8; 0];

        msg.as_ref().chunks(65535 - 16).for_each(|chunk| {
            let mut tmp_buf = vec![0u8; 65535];
            len += self.session
                .write_message( chunk,&mut tmp_buf)
                .unwrap();
            write_to_buf.extend_from_slice(&tmp_buf);
        });

        let mut msg_len_buf = vec![0u8; 4];
        LittleEndian::write_u32(&mut msg_len_buf, len as u32);
        let write_to_buf = &write_to_buf[0..len];
        msg_len_buf.extend_from_slice(write_to_buf);
        buf.extend_from_slice(&msg_len_buf);
        Ok(())
    }
}
