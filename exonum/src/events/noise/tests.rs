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

use futures::sync::{mpsc, mpsc::Receiver, mpsc::Sender};
use futures::{done, Future, Sink, Stream};
use snow::{types::Dh, wrappers::crypto_wrapper::Dh25519, NoiseBuilder};
use tokio_core::net::{TcpListener, TcpStream};
use tokio_core::reactor::Core;
use tokio_timer::{TimeoutStream, Timer};

use std::error::Error;
use std::io::{self, Result as IoResult};
use std::net::SocketAddr;
use std::thread;
use std::time::Duration;

use crypto::{gen_keypair, gen_keypair_from_seed, x25519::into_x25519_keypair, Seed,
             PUBLIC_KEY_LENGTH};
use events::error::{into_other, log_error};
use events::noise::wrapper::{NoiseWrapper, NOISE_MAX_MESSAGE_LENGTH};
use events::noise::{Handshake, HandshakeChannel, HandshakeParams, HandshakeResult, NoiseHandshake};

#[test]
fn test_convert_ed_to_curve_dh() {
    // Generate Ed25519 keys for initiator and responder.
    let (public_key_i, secret_key_i) = gen_keypair();
    let (public_key_r, secret_key_r) = gen_keypair();

    // Convert to Curve25519 keys.
    let (public_key_i, secret_key_i) = into_x25519_keypair(public_key_i, secret_key_i).unwrap();
    let (public_key_r, secret_key_r) = into_x25519_keypair(public_key_r, secret_key_r).unwrap();

    // Do DH.
    let mut keypair_i: Dh25519 = Default::default();
    keypair_i.set(secret_key_i.as_ref());
    let mut output_i = [0u8; PUBLIC_KEY_LENGTH];
    keypair_i.dh(public_key_r.as_ref(), &mut output_i);

    let mut keypair_r: Dh25519 = Default::default();
    keypair_r.set(secret_key_r.as_ref());
    let mut output_r = [0u8; PUBLIC_KEY_LENGTH];
    keypair_r.dh(public_key_i.as_ref(), &mut output_r);

    assert_eq!(output_i, output_r);
}

#[test]
fn test_converted_keys_handshake() {
    const MSG_SIZE: usize = 4096;
    static PATTERN: &'static str = "Noise_XK_25519_ChaChaPoly_SHA256";

    // Handshake initiator keypair.
    let (public_key_i, secret_key_i) = gen_keypair();
    // Handshake responder keypair.
    let (public_key_r, secret_key_r) = gen_keypair();

    // Convert to Curve25519 keys.
    let (_, secret_key_i) = into_x25519_keypair(public_key_i, secret_key_i).unwrap();
    let (public_key_r, secret_key_r) = into_x25519_keypair(public_key_r, secret_key_r).unwrap();

    let mut h_i = NoiseBuilder::new(PATTERN.parse().unwrap())
        .local_private_key(secret_key_i.as_ref())
        .remote_public_key(public_key_r.as_ref())
        .build_initiator()
        .expect("Unable to create initiator");

    let mut h_r = NoiseBuilder::new(PATTERN.parse().unwrap())
        .local_private_key(secret_key_r.as_ref())
        .build_responder()
        .expect("Unable to create responder");

    let mut buffer_msg = [0u8; MSG_SIZE * 2];
    let mut buffer_out = [0u8; MSG_SIZE * 2];

    let len = h_i.write_message(&[0u8; 0], &mut buffer_msg).unwrap();
    h_r.read_message(&buffer_msg[..len], &mut buffer_out)
        .unwrap();
    let len = h_r.write_message(&[0u8; 0], &mut buffer_msg).unwrap();
    h_i.read_message(&buffer_msg[..len], &mut buffer_out)
        .unwrap();
    let len = h_i.write_message(&[0u8; 0], &mut buffer_msg).unwrap();
    h_r.read_message(&buffer_msg[..len], &mut buffer_out)
        .unwrap();

    h_r.into_transport_mode()
        .expect("Unable to transition session into transport mode");
}

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum HandshakeStep {
    None,
    EphemeralKeyExchange(u8, usize),
    StaticKeyExchange(u8, usize),
}

const MAX_MESSAGE_LEN: usize = 128;

const EMPTY_MESSAGE: usize = 0;
const STANDARD_MESSAGE: usize = MAX_MESSAGE_LEN;
const LARGE_MESSAGE: usize = NOISE_MAX_MESSAGE_LENGTH + 1;

impl HandshakeParams {
    fn default_test_params() -> Self {
        let (public_key, secret_key) = gen_keypair_from_seed(&Seed::new([0; 32]));
        HandshakeParams {
            max_message_len: MAX_MESSAGE_LEN as u32,
            public_key,
            secret_key,
        }
    }
}

#[test]
#[should_panic(expected = "WrongMessageLength(0)")]
fn test_noise_handshake_errors_ee_empty() {
    let addr: SocketAddr = "127.0.0.1:45003".parse().unwrap();
    let step = HandshakeStep::EphemeralKeyExchange(1, EMPTY_MESSAGE);
    let (sender_err, listener_err) = wait_for_handshake_result(&addr, step);

    assert!(sender_err.is_err());
    listener_err.unwrap()
}

#[test]
#[should_panic(expected = "WrongMessageLength(0)")]
fn test_noise_handshake_errors_es_empty() {
    let addr: SocketAddr = "127.0.0.1:45004".parse().unwrap();
    let step = HandshakeStep::StaticKeyExchange(2, EMPTY_MESSAGE);
    let (sender_err, listener_err) = wait_for_handshake_result(&addr, step);

    assert!(sender_err.is_err());
    listener_err.unwrap()
}

#[test]
#[should_panic(expected = "HandshakeNotFinished")]
fn test_noise_handshake_errors_ee_standard() {
    let addr: SocketAddr = "127.0.0.1:45005".parse().unwrap();
    let step = HandshakeStep::EphemeralKeyExchange(1, STANDARD_MESSAGE);
    let (sender_err, listener_err) = wait_for_handshake_result(&addr, step);

    assert!(listener_err.is_err());
    sender_err.unwrap()
}

#[test]
#[should_panic(expected = "HandshakeNotFinished")]
fn test_noise_handshake_errors_es_standard() {
    let addr: SocketAddr = "127.0.0.1:45006".parse().unwrap();
    let step = HandshakeStep::StaticKeyExchange(2, STANDARD_MESSAGE);
    let (sender_err, listener_err) = wait_for_handshake_result(&addr, step);

    assert!(listener_err.is_err());
    sender_err.unwrap()
}

#[test]
#[should_panic(expected = "Message size exceeds max handshake message size")]
fn test_noise_handshake_errors_ee_large() {
    let addr: SocketAddr = "127.0.0.1:45007".parse().unwrap();
    let step = HandshakeStep::EphemeralKeyExchange(1, LARGE_MESSAGE);
    let (sender_err, listener_err) = wait_for_handshake_result(&addr, step);

    assert!(listener_err.is_err());
    sender_err.unwrap()
}

#[test]
#[should_panic(expected = "Message size exceeds max handshake message size")]
fn test_noise_handshake_errors_se_large() {
    let addr: SocketAddr = "127.0.0.1:45008".parse().unwrap();
    let step = HandshakeStep::StaticKeyExchange(2, LARGE_MESSAGE);
    let (sender_err, listener_err) = wait_for_handshake_result(&addr, step);

    assert!(listener_err.is_err());
    sender_err.unwrap()
}

// We need check result from both: sender and responder.
fn wait_for_handshake_result(
    addr: &SocketAddr,
    step: HandshakeStep,
) -> (IoResult<()>, IoResult<()>) {
    let addr2 = addr.clone();

    let (tx, rx) = mpsc::channel(0);
    let (err_tx, err_rx) = mpsc::channel::<IoResult<()>>(0);
    let rx = add_timeout_millis(rx, 500);
    let err_rx = add_timeout_millis(err_rx, 500);
    thread::spawn(move || run_handshake_listener(&addr2, tx, err_tx));

    // Use first handshake only to connect.
    let _res = send_handshake(&addr, HandshakeStep::None);
    rx.wait().next();

    let res = send_handshake(&addr, step);
    (res, err_rx.wait().next().unwrap().unwrap())
}

fn add_timeout_millis<T>(receiver: Receiver<T>, millis: u64) -> TimeoutStream<Receiver<T>> {
    let timer = Timer::default();
    timer.timeout_stream(receiver, Duration::from_millis(millis))
}

fn run_handshake_listener(
    addr: &SocketAddr,
    sender: Sender<()>,
    err_sender: Sender<IoResult<()>>,
) -> Result<(), io::Error> {
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
            let reader = handshake
                .and_then(|_| Ok(()))
                .or_else(|e| err_sender.send(Err(e)).map(|_| ()))
                .map_err(|e| log_error(e));
            handle.spawn(reader);
            Ok(())
        })
        .map_err(|e| into_other(e));

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
            }
            _ => {
                let error_handshake = NoiseErrorHandshake::new(step);
                error_handshake.send(&params, sock)
            }
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
    fn new(step: HandshakeStep, current_step: u8) -> Self {
        ErrorChannel { step, current_step }
    }
}

impl HandshakeChannel for ErrorChannel {
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
        let res = match &self.step {
            // Write message filled with zeros, instead of real handshake message.
            &HandshakeStep::EphemeralKeyExchange(cs, size)
            | &HandshakeStep::StaticKeyExchange(cs, size) if cs == self.current_step =>
            {
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
        NoiseErrorHandshake {
            channel: ErrorChannel::new(step, 1),
        }
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
