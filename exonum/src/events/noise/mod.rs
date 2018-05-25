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
use futures::future::{done, Future};
use tokio_core::net::TcpStream;
use tokio_io::{AsyncRead, codec::Framed, io::{read_exact, write_all}};

use std::io;

use crypto::{PublicKey, SecretKey};
use events::codec::MessagesCodec;
use events::noise::wrapper::{NoiseWrapper, HANDSHAKE_HEADER_LENGTH};

pub mod wrapper;
pub mod sodium_resolver;

type HandshakeResult = Box<Future<Item = Framed<TcpStream, MessagesCodec>, Error = io::Error>>;

#[derive(Debug, Clone)]
/// Params needed to establish secured connection using Noise Protocol.
pub struct HandshakeParams {
    pub public_key: PublicKey,
    pub secret_key: SecretKey,
    pub max_message_len: u32,
}

#[derive(Debug)]
pub struct NoiseHandshake {}

impl NoiseHandshake {
    pub fn listen(params: &HandshakeParams, stream: TcpStream) -> HandshakeResult {
        listen_handshake(stream, params)
    }

    pub fn send(params: &HandshakeParams, stream: TcpStream) -> HandshakeResult {
        send_handshake(stream, params)
    }
}

fn listen_handshake(stream: TcpStream, params: &HandshakeParams) -> HandshakeResult {
    let max_message_len = params.max_message_len;
    let mut noise = NoiseWrapper::responder(params);
    let framed = read(stream).and_then(move |(stream, msg)| {
        let _buf = noise.read_handshake_msg(&msg);
        write_handshake_msg(&mut noise)
            .and_then(|(len, buf)| write(stream, &buf, len))
            .and_then(|(stream, _msg)| read(stream))
            .and_then(move |(stream, msg)| {
                let _buf = noise.read_handshake_msg(&msg);
                let noise = noise.into_transport_mode()?;
                let framed = stream.framed(MessagesCodec::new(max_message_len, noise));
                Ok(framed)
            })
    });

    Box::new(framed)
}

fn send_handshake(stream: TcpStream, params: &HandshakeParams) -> HandshakeResult {
    let max_message_len = params.max_message_len;
    let mut noise = NoiseWrapper::initiator(params);
    let framed = write_handshake_msg(&mut noise)
        .and_then(|(len, buf)| write(stream, &buf, len))
        .and_then(|(stream, _msg)| read(stream))
        .and_then(move |(stream, msg)| {
            let _buf = noise.read_handshake_msg(&msg);
            write_handshake_msg(&mut noise)
                .and_then(|(len, buf)| write(stream, &buf, len))
                .and_then(move |(stream, _msg)| {
                    let noise = noise.into_transport_mode()?;
                    let framed = stream.framed(MessagesCodec::new(max_message_len, noise));
                    Ok(framed)
                })
        });

    Box::new(framed)
}

fn read(sock: TcpStream) -> Box<Future<Item = (TcpStream, Vec<u8>), Error = io::Error>> {
    let buf = vec![0u8; HANDSHAKE_HEADER_LENGTH];
    Box::new(
        read_exact(sock, buf)
            .and_then(|(stream, msg)| read_exact(stream, vec![0u8; msg[0] as usize])),
    )
}

fn write(
    sock: TcpStream,
    buf: &[u8],
    len: usize,
) -> Box<Future<Item = (TcpStream, Vec<u8>), Error = io::Error>> {
    let mut message = vec![0u8; HANDSHAKE_HEADER_LENGTH];
    LittleEndian::write_u16(&mut message, len as u16);
    message.extend_from_slice(&buf[0..len]);
    Box::new(write_all(sock, message))
}

fn write_handshake_msg(
    noise: &mut NoiseWrapper,
) -> Box<Future<Item = (usize, Vec<u8>), Error = io::Error>> {
    let res = noise.write_handshake_msg();
    Box::new(done(res.map_err(|e| e.into())))
}
