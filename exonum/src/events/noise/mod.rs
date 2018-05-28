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
        read_handshake_msg(&msg, &mut noise).and_then(move |_| {
            write_handshake_msg(&mut noise)
                .and_then(|(len, buf)| write(stream, &buf, len))
                .and_then(|(stream, _msg)| read(stream))
                .and_then(move |(stream, msg)| {
                    noise.read_handshake_msg(&msg)?;
                    let noise = noise.into_transport_mode()?;
                    let framed = stream.framed(MessagesCodec::new(max_message_len, noise));
                    Ok(framed)
                })
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
            read_handshake_msg(&msg, &mut noise).and_then(move |_| {
                write_handshake_msg(&mut noise)
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

pub fn read_handshake_msg(
    input: &[u8],
    noise: &mut NoiseWrapper,
) -> Box<Future<Item = (usize, Vec<u8>), Error = io::Error>> {
    let res = noise.read_handshake_msg(input);
    Box::new(done(res.map_err(|e| e.into())))
}

#[cfg(test)]
mod tests {
    use tokio_core::reactor::Core;
    use tokio_core::net::{TcpListener, TcpStream};
    use tokio_io::AsyncRead;
    use futures::{done, Future, Sink, Stream};
    use futures::sync::{mpsc, mpsc::Sender};

    use std::net::SocketAddr;
    use std::thread;
    use std::io;

    use events::noise::wrapper::{NoiseWrapper, NOISE_MAX_MESSAGE_LENGTH,
                                 NOISE_MIN_HANDSHAKE_MESSAGE_LENGTH};
    use events::noise::{read, read_handshake_msg, write, HandshakeParams, HandshakeResult,
                        NoiseHandshake};
    use events::error::{into_other, log_error};
    use events::codec::MessagesCodec;
    use crypto::{gen_keypair_from_seed, Seed};

    #[derive(Debug, PartialEq, Copy, Clone)]
    pub enum HandshakeStep {
        Default,
        One(u8, usize),
        Two(u8, usize),
    }

    const EMPTY_MESSAGE: usize = 0;
    const SMALL_MESSAGE: usize = NOISE_MIN_HANDSHAKE_MESSAGE_LENGTH - 1;
    const BIG_MESSAGE: usize = NOISE_MAX_MESSAGE_LENGTH + 1;

    impl Default for HandshakeParams {
        fn default() -> Self {
            let (public_key, secret_key) = gen_keypair_from_seed(&Seed::new([0; 32]));
            HandshakeParams {
                max_message_len: 1024,
                public_key,
                secret_key,
            }
        }
    }

    #[test]
    fn test_noise_normal_handshake() {
        let addr: SocketAddr = "127.0.0.1:45001".parse().unwrap();
        let addr2 = addr.clone();

        let (sender, receiver) = mpsc::channel(1);
        thread::spawn(move || run_handshake_listener(&addr2, sender));

        // Use first handshake only to connect.
        let _res = send_handshake(&addr, HandshakeStep::Default);
        receiver.wait().next();

        let res = send_handshake(&addr, HandshakeStep::Default);
        assert!(res.is_ok());
    }

    #[test]
    fn test_noise_handshake_errors() {
        let addr: SocketAddr = "127.0.0.1:45002".parse().unwrap();
        let addr2 = addr.clone();

        let (sender, receiver) = mpsc::channel(1);
        thread::spawn(move || run_handshake_listener(&addr2, sender));

        // Use first handshake only to connect.
        let _res = send_handshake(&addr, HandshakeStep::Default);
        receiver.wait().next();

        let res = send_handshake(&addr, HandshakeStep::One(1, EMPTY_MESSAGE));
        assert!(res.is_err());

        let res = send_handshake(&addr, HandshakeStep::Two(2, EMPTY_MESSAGE));
        assert!(res.is_err());

        let res = send_handshake(&addr, HandshakeStep::One(1, SMALL_MESSAGE));
        assert!(res.is_err());

        let res = send_handshake(&addr, HandshakeStep::Two(2, SMALL_MESSAGE));
        assert!(res.is_err());

        let res = send_handshake(&addr, HandshakeStep::One(1, BIG_MESSAGE));
        assert!(res.is_err());

        let res = send_handshake(&addr, HandshakeStep::Two(2, BIG_MESSAGE));
        assert!(res.is_err());
    }

    fn run_handshake_listener(addr: &SocketAddr, sender: Sender<()>) -> Result<(), io::Error> {
        let mut core = Core::new().unwrap();
        let handle = core.handle();
        let params = HandshakeParams::default();

        let fut_stream = TcpListener::bind(addr, &handle).unwrap();
        let fut = fut_stream
            .incoming()
            .for_each(|(stream, _)| {
                let sender = sender.clone();
                let send = sender.send(()).map(|_| ()).map_err(log_error);
                handle.spawn(send);

                let handshake = NoiseHandshake::listen(&params, stream);
                let reader = handshake.and_then(|_| Ok(())).map_err(log_error);
                handle.spawn(reader);
                Ok(())
            })
            .map_err(into_other);

        core.run(fut)
    }

    fn send_handshake(addr: &SocketAddr, step: HandshakeStep) -> Result<(), io::Error> {
        let mut core = Core::new().unwrap();
        let handle = core.handle();
        let params = HandshakeParams::default();

        let stream = TcpStream::connect(&addr, &handle)
            .and_then(|sock| match step {
                HandshakeStep::Default => NoiseHandshake::send(&params, sock),
                _ => noise_send_handshake_with_error(&params, sock, step),
            })
            .map(|_| ())
            .map_err(into_other);

        core.run(stream)
    }

    fn noise_send_handshake_with_error(
        params: &HandshakeParams,
        stream: TcpStream,
        step: HandshakeStep,
    ) -> HandshakeResult {
        let max_message_len = params.max_message_len;
        let mut noise = NoiseWrapper::initiator(params);
        let framed = write_handshake_msg_with_error(&mut noise, 1, &step)
            .and_then(|(len, buf)| write(stream, &buf, len))
            .and_then(|(stream, _msg)| read(stream))
            .and_then(move |(stream, msg)| {
                read_handshake_msg(&msg, &mut noise).and_then(move |_| {
                    write_handshake_msg_with_error(&mut noise, 2, &step)
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

    fn write_handshake_msg_with_error(
        noise: &mut NoiseWrapper,
        current_step: u8,
        step: &HandshakeStep,
    ) -> Box<Future<Item = (usize, Vec<u8>), Error = io::Error>> {
        let res = match step {
            &HandshakeStep::One(cs, size) | &HandshakeStep::Two(cs, size) if cs == current_step => {
                Ok((size, vec![0; size]))
            }
            _ => noise.write_handshake_msg(),
        };

        Box::new(done(res.map_err(|e| e.into())))
    }
}
