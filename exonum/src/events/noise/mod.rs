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

use std::io;
use crypto::PublicKey;
use events::{codec::MessagesCodec, noise::noise_codec::NoiseCodec, noise::wrapper::HEADER_LEN};
use tokio_core::net::TcpStream;
use tokio_io::{AsyncRead, codec::Framed};
use futures::future::{Future, ok};

mod noise_codec;
mod wrapper;

#[derive(Debug)]
pub struct NoiseHandshake {
    pub max_message_len: u32,
}

#[cfg(all(feature = "noise_protocol"))]
impl NoiseHandshake {
    pub fn listen(&self, stream: TcpStream, stored: &PublicKey) -> Box<Future<Item=Framed<TcpStream, NoiseCodec>, Error=io::Error>> {
        internal::listen_handshake(stream, stored, self.max_message_len)
    }

    pub fn send(&self, stream: TcpStream, stored: &PublicKey) -> Box<Future<Item=Framed<TcpStream, NoiseCodec>, Error=io::Error>> {
        internal::send_handshake(stream, stored, self.max_message_len)
    }
}

#[cfg(not(feature = "noise_protocol"))]
impl NoiseHandshake {
    pub fn listen(&self, stream: TcpStream, _: &PublicKey) -> Box<Future<Item=Framed<TcpStream, MessagesCodec>, Error=io::Error>> {
        self.framed_stream(stream)
    }

    pub fn send(&self, stream: TcpStream, _: &PublicKey) -> Box<Future<Item=Framed<TcpStream, MessagesCodec>, Error=io::Error>> {
        self.framed_stream(stream)
    }

    pub fn framed_stream(&self, stream: TcpStream) -> Box<Future<Item=Framed<TcpStream, MessagesCodec>, Error=io::Error>> {
        let framed = stream.framed(MessagesCodec::new(self.max_message_len));
        Box::new(ok(framed))
    }
}

mod internal {
    use std::io;
    use crypto::PublicKey;
    use events::noise::{wrapper::NoiseWrapper, noise_codec::NoiseCodec};
    use snow::{NoiseBuilder, params::NoiseParams};
    use tokio_core::net::TcpStream;
    use tokio_io::{AsyncRead, codec::Framed, io::{read_exact, write_all}};
    use futures::future::Future;
    use events::noise::wrapper::HEADER_LEN;
    use byteorder::{ByteOrder, LittleEndian, BigEndian};
    use events::noise::wrapper::HANDSHAKE_HEADER_LEN;

    //TODO: Consider using tokio-proto for noise handshake
    pub fn listen_handshake(stream: TcpStream,
                            stored: &PublicKey,
                            max_message_len: u32,
    ) -> Box<Future<Item=Framed<TcpStream, NoiseCodec>, Error=io::Error>> {
        let mut noise = NoiseWrapper::responder();
        let framed = read(stream)
            .and_then(move |(stream, msg)| {
                let buf = noise.red_handshake_msg(msg);
                // -> e, ee, s, es
                let (len, buf) = noise.write_handshake_msg().unwrap();
                write(stream, buf, len)
                    .and_then(|(stream, _msg)| {
                        read(stream)
                    })
                    .and_then(move |(stream, msg)| {
                        let buf = noise.red_handshake_msg(msg);
                        let mut noise = noise.into_transport_mode().unwrap();
                        let framed = stream.framed(NoiseCodec::new(noise, max_message_len));
                        Ok(framed)
                    })
            });

        Box::new(framed)
    }

    pub fn send_handshake(stream: TcpStream,
                          stored: &PublicKey,
                          max_message_len: u32,
    ) -> Box<Future<Item=Framed<TcpStream, NoiseCodec>, Error=io::Error>> {
        let mut noise = NoiseWrapper::initiator();
        // -> e
        let (len, buf) = noise.write_handshake_msg().unwrap();
        let framed
        = write(stream, buf, len)
            .and_then(|(stream, _msg)| {
                read(stream)
            })
            .and_then(move |(stream, msg)| {
                let buf = noise.red_handshake_msg(msg);
                let (len, buf) = noise.write_handshake_msg().unwrap();
                write(stream, buf, len)
                    .and_then(move|(stream, _msg)| {
                        let mut noise = noise.into_transport_mode().unwrap();
                        let framed = stream.framed(NoiseCodec::new(noise, max_message_len));
                        Ok(framed)
                    })
            });

        Box::new(framed)
    }

    fn read(sock: TcpStream) -> Box<Future<Item=(TcpStream, Vec<u8>), Error=io::Error>> {
        let buf = vec![0u8; HANDSHAKE_HEADER_LEN];
        Box::new(
            read_exact(sock, buf)
                .and_then(|(stream, msg)| read_exact(stream, vec![0u8; msg[0] as usize])),
        )
    }

    fn write(sock: TcpStream,
             buf: Vec<u8>,
             len: usize,
    ) -> Box<Future<Item=(TcpStream, Vec<u8>), Error=io::Error>> {
        let mut msg_len_buf = vec![0u8; HANDSHAKE_HEADER_LEN];
        LittleEndian::write_u16(&mut msg_len_buf, len as u16);
        let buf = &buf[0..len];
        msg_len_buf.extend_from_slice(buf);
        Box::new(write_all(sock, msg_len_buf))
    }
}
