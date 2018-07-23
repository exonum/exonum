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

use futures::{
    future::Either, sync::{mpsc, mpsc::Sender}, Future, Sink, Stream,
};
use snow::{types::Dh, NoiseBuilder};
use tokio_core::{
    net::{TcpListener, TcpStream}, reactor::Core,
};
use tokio_io::{AsyncRead, AsyncWrite};

use std::{
    error::Error, io::{self, Result as IoResult}, net::SocketAddr, thread, time::Duration,
};

use crypto::{gen_keypair_from_seed, x25519, Seed, PUBLIC_KEY_LENGTH, SEED_LENGTH};
use events::{
    error::into_other,
    noise::{
        wrappers::sodium_wrapper::resolver::SodiumDh25519, Handshake, HandshakeParams,
        HandshakeRawMessage, HandshakeResult, NoiseHandshake,
    },
};

#[test]
#[cfg(feature = "sodiumoxide-crypto")]
fn test_convert_ed_to_curve_dh() {
    use crypto::{gen_keypair, x25519::into_x25519_keypair};

    // Generate Ed25519 keys for initiator and responder.
    let (public_key_i, secret_key_i) = gen_keypair();
    let (public_key_r, secret_key_r) = gen_keypair();

    // Convert to Curve25519 keys.
    let (public_key_i, secret_key_i) = into_x25519_keypair(public_key_i, secret_key_i).unwrap();
    let (public_key_r, secret_key_r) = into_x25519_keypair(public_key_r, secret_key_r).unwrap();

    // Do DH.
    let mut keypair_i: SodiumDh25519 = Default::default();
    keypair_i.set(secret_key_i.as_ref());
    let mut output_i = [0_u8; PUBLIC_KEY_LENGTH];
    keypair_i.dh(public_key_r.as_ref(), &mut output_i);

    let mut keypair_r: SodiumDh25519 = Default::default();
    keypair_r.set(secret_key_r.as_ref());
    let mut output_r = [0_u8; PUBLIC_KEY_LENGTH];
    keypair_r.dh(public_key_i.as_ref(), &mut output_r);

    assert_eq!(output_i, output_r);
}

#[test]
#[cfg(feature = "sodiumoxide-crypto")]
fn test_converted_keys_handshake() {
    use crypto::{gen_keypair, x25519::into_x25519_keypair};

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

    let mut buffer_msg = [0_u8; MSG_SIZE * 2];
    let mut buffer_out = [0_u8; MSG_SIZE * 2];

    let len = h_i.write_message(&[0_u8; 0], &mut buffer_msg).unwrap();
    h_r.read_message(&buffer_msg[..len], &mut buffer_out)
        .unwrap();
    let len = h_r.write_message(&[0_u8; 0], &mut buffer_msg).unwrap();
    h_i.read_message(&buffer_msg[..len], &mut buffer_out)
        .unwrap();
    let len = h_i.write_message(&[0_u8; 0], &mut buffer_msg).unwrap();
    h_r.read_message(&buffer_msg[..len], &mut buffer_out)
        .unwrap();

    h_r.into_transport_mode()
        .expect("Unable to transition session into transport mode");
}

#[derive(Debug, Copy, Clone)]
struct BogusMessage {
    step: HandshakeStep,
    message: &'static [u8],
}

impl BogusMessage {
    fn new(step: HandshakeStep, message: &'static [u8]) -> Self {
        BogusMessage { step, message }
    }
}

#[derive(Debug, PartialEq, Copy, Clone)]
enum HandshakeStep {
    EphemeralKeyExchange,
    StaticKeyExchange,
    Done,
}

impl HandshakeStep {
    fn next(&self) -> Option<HandshakeStep> {
        use self::HandshakeStep::*;

        match *self {
            EphemeralKeyExchange => Some(StaticKeyExchange),
            StaticKeyExchange => Some(Done),
            Done => None,
        }
    }
}

const MAX_MESSAGE_LEN: usize = 128;

const EMPTY_MESSAGE: &[u8] = &[0; 0];
const STANDARD_MESSAGE: &[u8] = &[0; MAX_MESSAGE_LEN];

pub fn default_test_params() -> HandshakeParams {
    let (public_key, secret_key) = gen_keypair_from_seed(&Seed::new([1; SEED_LENGTH]));
    let mut params = HandshakeParams::new(public_key, secret_key, 1024);
    params.set_remote_key(public_key);
    params
}

#[test]
fn test_noise_handshake_errors_ee_empty() {
    let addr: SocketAddr = "127.0.0.1:45003".parse().unwrap();
    let params = default_test_params();
    let bogus_message = Some(BogusMessage::new(
        HandshakeStep::EphemeralKeyExchange,
        EMPTY_MESSAGE,
    ));
    let (_, listener_err) = wait_for_handshake_result(addr, &params, bogus_message, None);

    assert!(
        listener_err
            .unwrap_err()
            .description()
            .contains("WrongMessageLength")
    );
}

#[test]
fn test_noise_handshake_errors_es_empty() {
    let addr: SocketAddr = "127.0.0.1:45004".parse().unwrap();
    let params = default_test_params();
    let bogus_message = Some(BogusMessage::new(
        HandshakeStep::StaticKeyExchange,
        EMPTY_MESSAGE,
    ));
    let (_, listener_err) = wait_for_handshake_result(addr, &params, bogus_message, None);

    assert!(
        listener_err
            .unwrap_err()
            .description()
            .contains("WrongMessageLength")
    );
}

#[test]
fn test_noise_handshake_errors_ee_standard() {
    let addr: SocketAddr = "127.0.0.1:45005".parse().unwrap();
    let params = default_test_params();
    let bogus_message = Some(BogusMessage::new(
        HandshakeStep::EphemeralKeyExchange,
        STANDARD_MESSAGE,
    ));
    let (_, listener_err) = wait_for_handshake_result(addr, &params, bogus_message, None);

    assert!(listener_err.unwrap_err().description().contains("Decrypt"));
}

#[test]
fn test_noise_handshake_errors_es_standard() {
    let addr: SocketAddr = "127.0.0.1:45006".parse().unwrap();
    let params = default_test_params();
    let bogus_message = Some(BogusMessage::new(
        HandshakeStep::StaticKeyExchange,
        STANDARD_MESSAGE,
    ));
    let (_, listener_err) = wait_for_handshake_result(addr, &params, bogus_message, None);

    assert!(listener_err.unwrap_err().description().contains("Decrypt"));
}

#[test]
fn test_noise_handshake_errors_ee_empty_listen() {
    let addr: SocketAddr = "127.0.0.1:45007".parse().unwrap();
    let params = default_test_params();
    let bogus_message = Some(BogusMessage::new(
        HandshakeStep::EphemeralKeyExchange,
        EMPTY_MESSAGE,
    ));
    let (sender_err, _) = wait_for_handshake_result(addr, &params, None, bogus_message);

    assert!(
        sender_err
            .unwrap_err()
            .description()
            .contains("WrongMessageLength")
    );
}

#[test]
fn test_noise_handshake_errors_ee_standard_listen() {
    let addr: SocketAddr = "127.0.0.1:45008".parse().unwrap();
    let params = default_test_params();
    let bogus_message = Some(BogusMessage::new(
        HandshakeStep::EphemeralKeyExchange,
        STANDARD_MESSAGE,
    ));
    let (sender_err, _) = wait_for_handshake_result(addr, &params, None, bogus_message);

    assert!(sender_err.unwrap_err().description().contains("Decrypt"));
}

#[test]
fn test_noise_handshake_wrong_remote_key() {
    let addr: SocketAddr = "127.0.0.1:45009".parse().unwrap();
    let mut params = default_test_params();
    let (remote_key, _) = gen_keypair_from_seed(&Seed::new([2; SEED_LENGTH]));
    params.set_remote_key(remote_key);

    let (_, listener_err) = wait_for_handshake_result(addr, &params, None, None);

    assert!(listener_err.unwrap_err().description().contains("Decrypt"));
}

// We need check result from both: sender and responder.
fn wait_for_handshake_result(
    addr: SocketAddr,
    params: &HandshakeParams,
    sender_message: Option<BogusMessage>,
    responder_message: Option<BogusMessage>,
) -> (IoResult<()>, IoResult<()>) {
    let (err_tx, err_rx) = mpsc::channel::<io::Error>(0);

    let responder_message = responder_message.clone();
    let remote_params = params.clone();

    thread::spawn(move || run_handshake_listener(&addr, &remote_params, err_tx, responder_message));
    //TODO: very likely will be removed in [ECR-1664].
    thread::sleep(Duration::from_millis(500));

    let sender_err = send_handshake(&addr, &params, sender_message);
    let listener_err = err_rx
        .wait()
        .next()
        .expect("No listener error sent")
        .expect("Could not receive listener error");
    (sender_err, Err(listener_err))
}

fn run_handshake_listener(
    addr: &SocketAddr,
    params: &HandshakeParams,
    err_sender: Sender<io::Error>,
    bogus_message: Option<BogusMessage>,
) -> Result<(), io::Error> {
    let mut core = Core::new().unwrap();
    let handle = core.handle();

    core.run(
        TcpListener::bind(addr, &handle)
            .unwrap()
            .incoming()
            .for_each(move |(stream, _)| {
                let err_sender = err_sender.clone();

                handle.spawn({
                    let handshake = match bogus_message {
                        Some(message) => Either::A(
                            NoiseErrorHandshake::responder(&params, message).listen(stream),
                        ),
                        None => Either::B(NoiseHandshake::responder(&params).listen(stream)),
                    };

                    handshake
                        .map(|_| ())
                        .or_else(|e| err_sender.send(e).map(|_| ()))
                        .map_err(|e| panic!("{:?}", e))
                });
                Ok(())
            })
            .map_err(|e| into_other(e)),
    )
}

fn send_handshake(
    addr: &SocketAddr,
    params: &HandshakeParams,
    bogus_message: Option<BogusMessage>,
) -> Result<(), io::Error> {
    let mut core = Core::new().unwrap();
    let handle = core.handle();

    let stream = TcpStream::connect(&addr, &handle)
        .and_then(|sock| match bogus_message {
            None => NoiseHandshake::initiator(&params).send(sock),
            Some(message) => NoiseErrorHandshake::initiator(&params, message).send(sock),
        })
        .map(|_| ())
        .map_err(into_other);

    core.run(stream)
}

#[derive(Debug)]
struct NoiseErrorHandshake {
    bogus_message: BogusMessage,
    current_step: HandshakeStep,
    // Option is used in order to be able to move out `inner` from the instance.
    inner: Option<NoiseHandshake>,
}

impl NoiseErrorHandshake {
    fn initiator(params: &HandshakeParams, bogus_message: BogusMessage) -> Self {
        NoiseErrorHandshake {
            bogus_message,
            current_step: HandshakeStep::EphemeralKeyExchange,
            inner: Some(NoiseHandshake::initiator(params)),
        }
    }

    fn responder(params: &HandshakeParams, bogus_message: BogusMessage) -> Self {
        NoiseErrorHandshake {
            bogus_message,
            current_step: HandshakeStep::EphemeralKeyExchange,
            inner: Some(NoiseHandshake::responder(params)),
        }
    }

    fn read_handshake_msg<S: AsyncRead + 'static>(
        mut self,
        stream: S,
    ) -> impl Future<Item = (S, Self), Error = io::Error> {
        let inner = self.inner.take().unwrap();

        inner
            .read_handshake_msg(stream)
            .map(move |(stream, inner)| {
                self.inner = Some(inner);
                (stream, self)
            })
    }

    fn write_handshake_msg<S: AsyncWrite + 'static>(
        mut self,
        stream: S,
    ) -> impl Future<Item = (S, Self), Error = io::Error> {
        if self.current_step == self.bogus_message.step {
            let msg = self.bogus_message.message;

            Either::A(
                HandshakeRawMessage(msg.to_vec())
                    .write(stream)
                    .map(move |(stream, _)| {
                        self.current_step = self.current_step
                            .next()
                            .expect("Extra handshake step taken");
                        (stream, self)
                    }),
            )
        } else {
            let inner = self.inner.take().unwrap();

            Either::B(
                inner
                    .write_handshake_msg(stream)
                    .map(move |(stream, inner)| {
                        self.inner = Some(inner);
                        self.current_step = self.current_step
                            .next()
                            .expect("Extra handshake step taken");
                        (stream, self)
                    }),
            )
        }
    }
}

impl Handshake for NoiseErrorHandshake {
    type Result = x25519::PublicKey;

    fn listen<S>(self, stream: S) -> HandshakeResult<S, Self::Result>
    where
        S: AsyncRead + AsyncWrite + 'static,
    {
        let framed = self.read_handshake_msg(stream)
            .and_then(|(stream, handshake)| handshake.write_handshake_msg(stream))
            .and_then(|(stream, handshake)| handshake.read_handshake_msg(stream))
            .and_then(|(stream, handshake)| handshake.inner.unwrap().finalize(stream));
        Box::new(framed)
    }

    fn send<S>(self, stream: S) -> HandshakeResult<S, Self::Result>
    where
        S: AsyncRead + AsyncWrite + 'static,
    {
        let framed = self.write_handshake_msg(stream)
            .and_then(|(stream, handshake)| handshake.read_handshake_msg(stream))
            .and_then(|(stream, handshake)| handshake.write_handshake_msg(stream))
            .and_then(|(stream, handshake)| handshake.inner.unwrap().finalize(stream));
        Box::new(framed)
    }
}
