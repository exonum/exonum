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
use futures::future::{done, err, Future};
use tokio_core::net::TcpStream;
use tokio_io::{codec::Framed,
               io::{read_exact, write_all},
               AsyncRead};

use std::io;

use crypto::{PublicKey, SecretKey};
use events::noise::wrapper::NOISE_MAX_HANDSHAKE_MESSAGE_LENGTH;
use events::{codec::MessagesCodec,
             noise::wrapper::{NoiseWrapper, HANDSHAKE_HEADER_LENGTH}};

pub mod wrapper;

#[cfg(test)]
mod tests;

type HandshakeResult = Box<Future<Item = Framed<TcpStream, MessagesCodec>, Error = io::Error>>;

#[derive(Debug, Clone)]
/// Params needed to establish secured connection using Noise Protocol.
pub struct HandshakeParams {
    pub public_key: PublicKey,
    pub secret_key: SecretKey,
    pub max_message_len: u32,
}

pub trait Handshake {
    fn listen(self, params: &HandshakeParams, stream: TcpStream) -> HandshakeResult;
    fn send(self, params: &HandshakeParams, stream: TcpStream) -> HandshakeResult;
}

#[derive(Debug)]
pub struct NoiseHandshake {
    channel: NoiseHandshakeChannel,
}

impl NoiseHandshake {
    pub fn new() -> Self {
        NoiseHandshake {
            channel: Default::default(),
        }
    }
}

impl Handshake for NoiseHandshake {
    fn listen(self, params: &HandshakeParams, stream: TcpStream) -> HandshakeResult {
        listen_handshake(stream, &params, &self.channel)
    }

    fn send(self, params: &HandshakeParams, stream: TcpStream) -> HandshakeResult {
        send_handshake(stream, &params, &self.channel)
    }
}

pub trait HandshakeChannel {
    fn read_handshake_msg(
        &self,
        msg: &[u8],
        noise: &mut NoiseWrapper,
    ) -> Box<Future<Item = (usize, Vec<u8>), Error = io::Error>>;
    fn write_handshake_msg(
        &mut self,
        noise: &mut NoiseWrapper,
    ) -> Box<Future<Item = (usize, Vec<u8>), Error = io::Error>>;
}

#[derive(Debug, Default, Clone)]
pub struct NoiseHandshakeChannel {}

impl HandshakeChannel for NoiseHandshakeChannel {
    fn read_handshake_msg(
        &self,
        input: &[u8],
        noise: &mut NoiseWrapper,
    ) -> Box<Future<Item = (usize, Vec<u8>), Error = io::Error>> {
        let res = noise.read_handshake_msg(input);
        Box::new(done(res.map_err(|e| e.into())))
    }

    fn write_handshake_msg(
        &mut self,
        noise: &mut NoiseWrapper,
    ) -> Box<Future<Item = (usize, Vec<u8>), Error = io::Error>> {
        let res = noise.write_handshake_msg();
        Box::new(done(res.map_err(|e| e.into())))
    }
}

fn listen_handshake<T>(stream: TcpStream, params: &HandshakeParams, channel: &T) -> HandshakeResult
where
    T: HandshakeChannel + Clone + 'static,
{
    let max_message_len = params.max_message_len;
    let mut noise = NoiseWrapper::responder(params);

    let mut channel = channel.clone();
    let framed = read(stream).and_then(move |(stream, msg)| {
        channel
            .read_handshake_msg(&msg, &mut noise)
            .and_then(move |_| {
                channel
                    .write_handshake_msg(&mut noise)
                    .and_then(|(len, buf)| write(stream, &buf, len))
                    .and_then(|(stream, _msg)| read(stream))
                    .and_then(move |(stream, msg)| {
                        channel
                            .read_handshake_msg(&msg, &mut noise)
                            .and_then(move |_| {
                                let noise = noise.into_transport_mode()?;
                                let framed =
                                    stream.framed(MessagesCodec::new(max_message_len, noise));
                                Ok(framed)
                            })
                    })
            })
    });

    Box::new(framed)
}

fn send_handshake<T>(stream: TcpStream, params: &HandshakeParams, channel: &T) -> HandshakeResult
where
    T: HandshakeChannel + Clone + 'static,
{
    let max_message_len = params.max_message_len;
    let mut noise = NoiseWrapper::initiator(params);
    let mut channel = channel.clone();
    let framed = channel
        .write_handshake_msg(&mut noise)
        .and_then(|(len, buf)| write(stream, &buf, len))
        .and_then(|(stream, _msg)| read(stream))
        .and_then(move |(stream, msg)| {
            channel
                .read_handshake_msg(&msg, &mut noise)
                .and_then(move |_| {
                    channel
                        .write_handshake_msg(&mut noise)
                        .and_then(|(len, buf)| write(stream, &buf, len))
                        .and_then(move |(stream, _msg)| {
                            let noise = noise.into_transport_mode()?;
                            let framed = stream.framed(MessagesCodec::new(max_message_len, noise));
                            Ok(framed)
                        })
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

    if len > NOISE_MAX_HANDSHAKE_MESSAGE_LENGTH {
        return Box::new(err(io::Error::new(
            io::ErrorKind::Other, "Message size exceeds max handshake message size"
        )));
    }

    LittleEndian::write_u16(&mut message, len as u16);
    message.extend_from_slice(&buf[0..len]);
    Box::new(write_all(sock, message))
}
