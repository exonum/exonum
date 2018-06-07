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
use events::noise::wrapper::NoiseError;

pub mod wrapper;

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
        NoiseHandshake { channel: Default::default() }
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
    fn read_handshake_msg(&self, msg: &[u8], noise: &mut NoiseWrapper) -> Box<Future<Item=(usize, Vec<u8>), Error=io::Error>>;
    fn write_handshake_msg(&mut self, noise: &mut NoiseWrapper) -> Box<Future<Item=(usize, Vec<u8>), Error=io::Error>>;
}

#[derive(Debug, Default, Clone)]
pub struct NoiseHandshakeChannel {}

impl HandshakeChannel for NoiseHandshakeChannel {
    fn read_handshake_msg(&self,
        input: &[u8],
        noise: &mut NoiseWrapper,
    ) -> Box<Future<Item = (usize, Vec<u8>), Error = io::Error>> {
        let res = noise.read_handshake_msg(input);
        Box::new(done(res.map_err(|e| e.into())))
    }

    fn write_handshake_msg(&mut self,
        noise: &mut NoiseWrapper,
    ) -> Box<Future<Item = (usize, Vec<u8>), Error = io::Error>> {
        let res = noise.write_handshake_msg();
        Box::new(done(res.map_err(|e| e.into())))
    }
}

fn listen_handshake<T>(stream: TcpStream, params: &HandshakeParams, channel: &T) -> HandshakeResult
where T: HandshakeChannel + Clone + 'static {
    let max_message_len = params.max_message_len;
    let mut noise = NoiseWrapper::responder(params);
    let mut channel = channel.clone();
    let framed = read(stream).and_then(move |(stream, msg)| {
        channel.read_handshake_msg(&msg, &mut noise).and_then(move |_| {
            channel.write_handshake_msg(&mut noise)
                .and_then(|(len, buf)| write(stream, &buf, len))
                .and_then(|(stream, _msg)| read(stream))
                .and_then(move |(stream, msg)| channel.read_handshake_msg(&msg, &mut noise)
                    .and_then(move |_|{
                        let noise = noise.into_transport_mode()?;
                        let framed = stream.framed(MessagesCodec::new(max_message_len, noise));
                        Ok(framed)
                }))
        })
    });

    Box::new(framed)
}

fn send_handshake<T>(stream: TcpStream, params: &HandshakeParams, channel: &T) -> HandshakeResult
where T: HandshakeChannel + Clone + 'static {
    let max_message_len = params.max_message_len;
    let mut noise = NoiseWrapper::initiator(params);
    let mut channel = channel.clone();
    let framed = channel.write_handshake_msg(&mut noise)
        .and_then(|(len, buf)| write(stream, &buf, len))
        .and_then(|(stream, _msg)| read(stream))
        .and_then(move |(stream, msg)| {
            channel.read_handshake_msg(&msg, &mut noise).and_then(move |_| {
                channel.write_handshake_msg(&mut noise)
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

#[cfg(test)]
mod tests {
    use futures::sync::{mpsc, mpsc::Receiver, mpsc::Sender};
    use futures::{done, Future, Sink, Stream};
    use tokio_core::net::{TcpListener, TcpStream};
    use tokio_core::reactor::Core;
    use tokio_timer::{TimeoutStream, Timer};

    use std::io;
    use std::net::SocketAddr;
    use std::thread;
    use std::time::Duration;

    use crypto::{gen_keypair_from_seed, Seed};
    use events::error::{into_other, log_error};
    use events::noise::wrapper::{NoiseWrapper, NOISE_MAX_MESSAGE_LENGTH,
                                 NOISE_MIN_HANDSHAKE_MESSAGE_LENGTH};
    use events::noise::{HandshakeParams, HandshakeResult,
                        NoiseHandshake, Handshake, HandshakeChannel};

    #[derive(Debug, PartialEq, Copy, Clone)]
    pub enum HandshakeStep {
        None,
        EphemeralKeyExchange(u8, usize),
        StaticKeyExchange(u8, usize),
    }

    const EMPTY_MESSAGE: usize = 0;
    const SMALL_MESSAGE: usize = NOISE_MIN_HANDSHAKE_MESSAGE_LENGTH + 1;
    const BIG_MESSAGE: usize = NOISE_MAX_MESSAGE_LENGTH + 1;

    impl HandshakeParams {
        fn default_test_params() -> Self {
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

        let (sender, receiver) = mpsc::channel(0);
        let (err_sender, err_receiver) = mpsc::channel::<io::Error>(0);
        let receiver = add_timeout_millis(receiver, 500);
        thread::spawn(move || run_handshake_listener(&addr2, sender.clone(), err_sender));

        // Use first handshake only to connect.
        let _res = send_handshake(&addr, HandshakeStep::None);
        receiver.wait().next();

        let res = send_handshake(&addr, HandshakeStep::None);
        assert!(res.is_ok());
    }

    #[test]
    fn test_noise_handshake_errors_2() {
        let addr: SocketAddr = "127.0.0.1:45003".parse().unwrap();
        let step = HandshakeStep::EphemeralKeyExchange(1, EMPTY_MESSAGE);

        let error = error_handshake_variant(&addr, step).unwrap();


        println!("error {:?}", error);
    }

    #[test]
    fn test_noise_handshake_errors_3() {
        let addr: SocketAddr = "127.0.0.1:45004".parse().unwrap();
        let step = HandshakeStep::StaticKeyExchange(2, EMPTY_MESSAGE);

        let error = error_handshake_variant(&addr, step).unwrap();

        println!("error {:?}", error);
    }

    #[test]
    fn test_noise_handshake_errors_6() {
        let addr: SocketAddr = "127.0.0.1:45005".parse().unwrap();
        let step = HandshakeStep::EphemeralKeyExchange(1, SMALL_MESSAGE);

        let error = error_handshake_variant(&addr, step).unwrap();

        println!("error {:?}", error);
    }

    #[test]
    fn test_noise_handshake_errors_7() {
        let addr: SocketAddr = "127.0.0.1:45006".parse().unwrap();
        let step = HandshakeStep::StaticKeyExchange(2, SMALL_MESSAGE);

        let error = error_handshake_variant(&addr, step).unwrap();

        println!("error {:?}", error);
    }

    #[test]
    fn test_noise_handshake_errors_4() {
        let addr: SocketAddr = "127.0.0.1:45007".parse().unwrap();
        let step = HandshakeStep::EphemeralKeyExchange(1, BIG_MESSAGE);

        let error = error_handshake_variant(&addr, step).unwrap();

        println!("error {:?}", error);
    }

    #[test]
    fn test_noise_handshake_errors_5() {
        let addr: SocketAddr = "127.0.0.1:45008".parse().unwrap();
        let step = HandshakeStep::StaticKeyExchange(2, BIG_MESSAGE);
        let error = error_handshake_variant(&addr, step).unwrap();

        println!("error {:?}", error);
    }

    fn error_handshake_variant(addr:&SocketAddr, step:HandshakeStep) -> Result<io::Error, ()> {
        let addr2 = addr.clone();

        let (sender, receiver) = mpsc::channel(0);
        let (err_sender, err_receiver) = mpsc::channel::<io::Error>(0);
        let receiver = add_timeout_millis(receiver, 500);
        let err_receiver = add_timeout_millis(err_receiver, 500);
        thread::spawn(move || run_handshake_listener(&addr2, sender, err_sender));

        // Use first handshake only to connect.
        let _res = send_handshake(&addr, HandshakeStep::None);
        receiver.wait().next();

        let res = send_handshake(&addr, step);
        assert!(res.is_err());

        err_receiver.wait().next().unwrap()
    }

    fn add_timeout_millis<T>(receiver: Receiver<T>, millis: u64) -> TimeoutStream<Receiver<T>> {
        let timer = Timer::default();
        timer.timeout_stream(receiver, Duration::from_millis(millis))
    }

    fn run_handshake_listener(addr: &SocketAddr, sender: Sender<()>, err_sender: Sender<io::Error>) -> Result<(), io::Error> {
        let mut core = Core::new().unwrap();
        let handle = core.handle();
        let params = HandshakeParams::default_test_params();

        let fut_stream = TcpListener::bind(addr, &handle).unwrap();
        let fut = fut_stream
            .incoming()
            .for_each(|(stream, _)| {
                let sender = sender.clone();
                let err_sender = err_sender.clone();
                let send = sender.send(()).map(|_| ()).map_err(log_error);
                handle.spawn(send);

                let handshake = NoiseHandshake::new();
                let handshake = handshake.listen(&params, stream);
                let reader = handshake.and_then(|_| Ok(())).or_else(|e| {
                    err_sender.send(e).map(|_| ())
                }).map_err(|e| {
                    println!("into_other {:?}", e);
                    log_error(e)
                });
                handle.spawn(reader);
                Ok(())
            })
            .map_err(|e| {
                into_other(e)
            });

        core.run(fut)
    }

    fn send_handshake(addr: &SocketAddr, step: HandshakeStep) -> Result<(), io::Error> {
        let mut core = Core::new().unwrap();
        let handle = core.handle();
        let params = HandshakeParams::default_test_params();

        let stream = TcpStream::connect(&addr, &handle)
            .and_then(|sock| match step {
                HandshakeStep::None => {
                    let handshake = NoiseHandshake::new();
                    handshake.send(&params, sock)
                },
                _ => {
                    let error_handshake = NoiseErrorHandshake::new(step);
                    error_handshake.send(&params, sock)
                },
            })
            .map(|_| ())
            .map_err(into_other);

        core.run(stream)
    }

    #[derive(Debug, Copy, Clone)]
    pub struct ErrorChannel {
        step: HandshakeStep,
        current_step: u8,
    }

    impl ErrorChannel {
        fn new(step:HandshakeStep, current_step:u8) -> Self{
            ErrorChannel { step, current_step }
        }
    }

    impl HandshakeChannel for ErrorChannel {

        fn read_handshake_msg(&self,
                              input: &[u8],
                              noise: &mut NoiseWrapper,
        ) -> Box<Future<Item = (usize, Vec<u8>), Error = io::Error>> {
            let res = noise.read_handshake_msg(input);
            Box::new(done(res.map_err(|e| e.into())))
        }

        fn write_handshake_msg(&mut self,
                               noise: &mut NoiseWrapper,
        ) -> Box<Future<Item = (usize, Vec<u8>), Error = io::Error>> {
            let res = match &self.step {
                // Write message filled with zeros, instead of real handshake message.
                &HandshakeStep::EphemeralKeyExchange(cs, size) |
                &HandshakeStep::StaticKeyExchange(cs, size) if cs == self.current_step => {
                    Ok((size, vec![0; size]))
                }
                _ => noise.write_handshake_msg(),
            };

            self.current_step += 1;
            Box::new(done(res.map_err(|e| e.into())))
        }
    }

    #[derive(Debug)]
    pub struct NoiseErrorHandshake {
        channel: ErrorChannel,
    }

    impl NoiseErrorHandshake {
        fn new(step: HandshakeStep) -> Self {
            NoiseErrorHandshake { channel: ErrorChannel::new(step, 1) }
        }
    }

    impl Handshake for NoiseErrorHandshake {
        fn listen(self, params: &HandshakeParams, stream: TcpStream) -> HandshakeResult {
            super::listen_handshake(stream, &params, &self.channel)
        }

        fn send(self, params: &HandshakeParams, stream: TcpStream) -> HandshakeResult {
            super::send_handshake(stream, &params, &self.channel)
        }
    }
}
