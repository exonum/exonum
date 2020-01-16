// Copyright 2020 The Exonum Team
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
use bytes::BytesMut;
use exonum::{
    crypto::{gen_keypair_from_seed, Seed, PUBLIC_KEY_LENGTH, SEED_LENGTH},
    merkledb::BinaryValue,
};
use futures::{
    future::Either,
    sync::{mpsc, mpsc::Sender},
    Future, Sink, Stream,
};
use pretty_assertions::assert_eq;
use snow::{types::Dh, Builder};
use tokio_core::{
    net::{TcpListener, TcpStream},
    reactor::Core,
};
use tokio_io::{AsyncRead, AsyncWrite};

use std::{net::SocketAddr, thread, time::Duration};

use crate::events::{
    error::into_failure,
    noise::{
        wrappers::sodium_wrapper::resolver::{SodiumDh25519, SodiumResolver},
        Handshake, HandshakeParams, HandshakeRawMessage, HandshakeResult, NoiseHandshake,
        NoiseWrapper, TransportWrapper, HEADER_LENGTH, MAX_MESSAGE_LENGTH,
    },
    tests::raw_message,
};

#[test]
#[cfg(feature = "exonum_sodiumoxide")]
fn noise_convert_ed_to_curve_dh() {
    use crate::crypto::{gen_keypair, x25519::into_x25519_keypair};

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
    keypair_i.dh(public_key_r.as_ref(), &mut output_i).unwrap();

    let mut keypair_r: SodiumDh25519 = Default::default();
    keypair_r.set(secret_key_r.as_ref());
    let mut output_r = [0_u8; PUBLIC_KEY_LENGTH];
    keypair_r.dh(public_key_i.as_ref(), &mut output_r).unwrap();

    assert_eq!(output_i, output_r);
}

#[test]
#[cfg(feature = "exonum_sodiumoxide")]
fn noise_converted_keys_handshake() {
    use crate::crypto::{gen_keypair, x25519::into_x25519_keypair};

    const MSG_SIZE: usize = 4096;
    const PATTERN: &str = "Noise_XK_25519_ChaChaPoly_SHA256";

    // Handshake initiator keypair.
    let (public_key_i, secret_key_i) = gen_keypair();
    // Handshake responder keypair.
    let (public_key_r, secret_key_r) = gen_keypair();

    // Convert to Curve25519 keys.
    let (_, secret_key_i) = into_x25519_keypair(public_key_i, secret_key_i).unwrap();
    let (public_key_r, secret_key_r) = into_x25519_keypair(public_key_r, secret_key_r).unwrap();

    let mut h_i = Builder::with_resolver(PATTERN.parse().unwrap(), Box::new(SodiumResolver))
        .local_private_key(secret_key_i.as_ref())
        .remote_public_key(public_key_r.as_ref())
        .build_initiator()
        .expect("Unable to create initiator");

    let mut h_r = Builder::with_resolver(PATTERN.parse().unwrap(), Box::new(SodiumResolver))
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

#[test]
fn noise_encrypt_decrypt_max_message_len() {
    let small_sizes = 0..100;

    // Message sizes that must be tested:
    // 1. 65_445 (MAX_MESSAGE_LENGTH - SIGNATURE_LENGTH - HEADER_LENGTH - 22)
    // because in this case `raw_message_len` is divisible by (MAX_MESSAGE_LENGTH - TAG_LENGTH)
    // 2. 65_446 (previous size + 1)
    // from this size message is being split.
    // 3. 130_964 - next message size when `raw_message_len` is divisible by
    // (MAX_MESSAGE_LENGTH - TAG_LENGTH)
    // 4. 130_965 - Size when message is being split by 3 chunks.
    // To be sure we also test ranges near zero and near MAX_MESSAGE_LENGTH.
    let lower_bound = MAX_MESSAGE_LENGTH - 100;
    let upper_bound = MAX_MESSAGE_LENGTH + 100;

    let near_max_sizes = lower_bound..upper_bound;
    let big_size = vec![130_964, 130_965];

    for size in small_sizes.chain(near_max_sizes).chain(big_size) {
        check_encrypt_decrypt_message(size);
    }
}

#[test]
fn noise_encrypt_decrypt_bogus_message() {
    let msg_size = 64;

    let (mut initiator, mut responder) = create_noise_sessions();
    let mut buffer_msg = BytesMut::with_capacity(msg_size);

    initiator
        .encrypt_msg(&vec![0_u8; msg_size], &mut buffer_msg)
        .expect("Unable to encrypt message");

    let len = LittleEndian::read_u32(&buffer_msg[..HEADER_LENGTH]) as usize;

    // Wrong length.
    let res = responder.decrypt_msg(len - 1, &mut buffer_msg);
    assert!(res.unwrap_err().to_string().contains("decrypt error"));

    // Wrong message.
    let res = responder.decrypt_msg(len, &mut BytesMut::from(vec![0_u8; len + HEADER_LENGTH]));
    assert!(res.unwrap_err().to_string().contains("decrypt error"));
}

fn check_encrypt_decrypt_message(msg_size: usize) {
    let (mut initiator, mut responder) = create_noise_sessions();
    let mut buffer_msg = BytesMut::with_capacity(msg_size);
    let message = raw_message(msg_size);

    initiator
        .encrypt_msg(&message.to_bytes(), &mut buffer_msg)
        .unwrap_or_else(|e| panic!("Unable to encrypt message with size {}: {}", msg_size, e));

    let len = LittleEndian::read_u32(&buffer_msg[..HEADER_LENGTH]) as usize;

    let res = responder
        .decrypt_msg(len, &mut buffer_msg)
        .unwrap_or_else(|e| panic!("Unable to decrypt message with size {}: {}", msg_size, e));
    assert_eq!(&message.to_bytes(), &res);
}

fn create_noise_sessions() -> (TransportWrapper, TransportWrapper) {
    let params = HandshakeParams::with_default_params();

    let mut initiator = NoiseWrapper::initiator(&params);
    let mut responder = NoiseWrapper::responder(&params);

    let buffer_out = initiator.write_handshake_msg(&[]).unwrap();
    responder.read_handshake_msg(&buffer_out).unwrap();

    let buffer_out = responder.write_handshake_msg(&[]).unwrap();
    initiator.read_handshake_msg(&buffer_out).unwrap();
    let buffer_out = initiator.write_handshake_msg(&[]).unwrap();
    responder.read_handshake_msg(&buffer_out).unwrap();

    (
        initiator
            .into_transport_wrapper()
            .expect("convert to transport wrapper"),
        responder
            .into_transport_wrapper()
            .expect("convert to transport wrapper"),
    )
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
    fn next(self) -> Option<HandshakeStep> {
        use self::HandshakeStep::*;

        match self {
            EphemeralKeyExchange => Some(StaticKeyExchange),
            StaticKeyExchange => Some(Done),
            Done => None,
        }
    }
}

const MAX_MESSAGE_LEN: usize = 128;

const EMPTY_MESSAGE: &[u8] = &[0; 0];
const STANDARD_MESSAGE: &[u8] = &[0; MAX_MESSAGE_LEN];

#[test]
#[should_panic(expected = "WrongMessageLength")]
fn test_noise_handshake_errors_ee_empty() {
    let addr: SocketAddr = "127.0.0.1:45003".parse().unwrap();
    let params = HandshakeParams::with_default_params();
    let bogus_message = Some(BogusMessage::new(
        HandshakeStep::EphemeralKeyExchange,
        EMPTY_MESSAGE,
    ));
    let (_, listener_err) = wait_for_handshake_result(addr, &params, bogus_message, None);

    listener_err.unwrap()
}

#[test]
#[should_panic(expected = "WrongMessageLength")]
fn test_noise_handshake_errors_es_empty() {
    let addr: SocketAddr = "127.0.0.1:45004".parse().unwrap();
    let params = HandshakeParams::with_default_params();
    let bogus_message = Some(BogusMessage::new(
        HandshakeStep::StaticKeyExchange,
        EMPTY_MESSAGE,
    ));
    let (_, listener_err) = wait_for_handshake_result(addr, &params, bogus_message, None);

    listener_err.unwrap()
}

#[test]
#[should_panic(expected = "Dh")]
fn test_noise_handshake_errors_ee_standard() {
    let addr: SocketAddr = "127.0.0.1:45005".parse().unwrap();
    let params = HandshakeParams::with_default_params();
    let bogus_message = Some(BogusMessage::new(
        HandshakeStep::EphemeralKeyExchange,
        STANDARD_MESSAGE,
    ));
    let (_, listener_err) = wait_for_handshake_result(addr, &params, bogus_message, None);

    listener_err.unwrap()
}

#[test]
#[should_panic(expected = "Decrypt")]
fn test_noise_handshake_errors_es_standard() {
    let addr: SocketAddr = "127.0.0.1:45006".parse().unwrap();
    let params = HandshakeParams::with_default_params();
    let bogus_message = Some(BogusMessage::new(
        HandshakeStep::StaticKeyExchange,
        STANDARD_MESSAGE,
    ));
    let (_, listener_err) = wait_for_handshake_result(addr, &params, bogus_message, None);

    listener_err.unwrap();
}

#[test]
#[should_panic(expected = "WrongMessageLength")]
fn test_noise_handshake_errors_ee_empty_listen() {
    let addr: SocketAddr = "127.0.0.1:45007".parse().unwrap();
    let params = HandshakeParams::with_default_params();
    let bogus_message = Some(BogusMessage::new(
        HandshakeStep::EphemeralKeyExchange,
        EMPTY_MESSAGE,
    ));
    let (sender_err, _) = wait_for_handshake_result(addr, &params, None, bogus_message);

    sender_err.unwrap();
}

#[test]
#[should_panic(expected = "Dh")]
fn test_noise_handshake_errors_ee_standard_listen() {
    let addr: SocketAddr = "127.0.0.1:45008".parse().unwrap();
    let params = HandshakeParams::with_default_params();
    let bogus_message = Some(BogusMessage::new(
        HandshakeStep::EphemeralKeyExchange,
        STANDARD_MESSAGE,
    ));
    let (sender_err, _) = wait_for_handshake_result(addr, &params, None, bogus_message);

    sender_err.unwrap();
}

#[test]
#[should_panic(expected = "Decrypt")]
fn test_noise_handshake_wrong_remote_key() {
    let addr: SocketAddr = "127.0.0.1:45009".parse().unwrap();
    let mut params = HandshakeParams::with_default_params();
    let (remote_key, _) = gen_keypair_from_seed(&Seed::new([2; SEED_LENGTH]));
    params.set_remote_key(remote_key);

    let (_, listener_err) = wait_for_handshake_result(addr, &params, None, None);

    listener_err.unwrap();
}

// We need check result from both: sender and responder.
fn wait_for_handshake_result(
    addr: SocketAddr,
    params: &HandshakeParams,
    sender_message: Option<BogusMessage>,
    responder_message: Option<BogusMessage>,
) -> (Result<(), failure::Error>, Result<(), failure::Error>) {
    let (err_tx, err_rx) = mpsc::channel::<failure::Error>(0);

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
    err_sender: Sender<failure::Error>,
    bogus_message: Option<BogusMessage>,
) -> Result<(), failure::Error> {
    let mut core = Core::new().unwrap();
    let handle = core.handle();

    core.run(
        TcpListener::bind(addr, &handle)
            .unwrap()
            .incoming()
            .for_each(move |(stream, peer)| {
                let err_sender = err_sender.clone();

                handle.spawn({
                    let handshake = match bogus_message {
                        Some(message) => Either::A(
                            NoiseErrorHandshake::responder(&params, &peer, message).listen(stream),
                        ),
                        None => Either::B(NoiseHandshake::responder(&params, &peer).listen(stream)),
                    };

                    handshake
                        .map(|_| ())
                        .or_else(|e| err_sender.send(e).map(|_| ()))
                        .map_err(|e| panic!("{:?}", e))
                });
                Ok(())
            })
            .map_err(into_failure),
    )
}

fn send_handshake(
    addr: &SocketAddr,
    params: &HandshakeParams,
    bogus_message: Option<BogusMessage>,
) -> Result<(), failure::Error> {
    let mut core = Core::new().unwrap();
    let handle = core.handle();

    let stream = TcpStream::connect(&addr, &handle)
        .map_err(into_failure)
        .and_then(|sock| match bogus_message {
            None => NoiseHandshake::initiator(&params, addr).send(sock),
            Some(message) => NoiseErrorHandshake::initiator(&params, addr, message).send(sock),
        })
        .map(|_| ());

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
    fn initiator(
        params: &HandshakeParams,
        peer_address: &SocketAddr,
        bogus_message: BogusMessage,
    ) -> Self {
        NoiseErrorHandshake {
            bogus_message,
            current_step: HandshakeStep::EphemeralKeyExchange,
            inner: Some(NoiseHandshake::initiator(params, peer_address)),
        }
    }

    fn responder(
        params: &HandshakeParams,
        peer_address: &SocketAddr,
        bogus_message: BogusMessage,
    ) -> Self {
        NoiseErrorHandshake {
            bogus_message,
            current_step: HandshakeStep::EphemeralKeyExchange,
            inner: Some(NoiseHandshake::responder(params, peer_address)),
        }
    }

    fn read_handshake_msg<S: AsyncRead + 'static>(
        mut self,
        stream: S,
    ) -> impl Future<Item = (S, Self), Error = failure::Error> {
        let inner = self.inner.take().unwrap();

        inner
            .read_handshake_msg(stream)
            .map(move |(stream, inner, _)| {
                self.inner = Some(inner);
                (stream, self)
            })
    }

    fn write_handshake_msg<S: AsyncWrite + 'static>(
        mut self,
        stream: S,
    ) -> impl Future<Item = (S, Self), Error = failure::Error> {
        if self.current_step == self.bogus_message.step {
            let msg = self.bogus_message.message;

            Either::A(
                HandshakeRawMessage(msg.to_vec())
                    .write(stream)
                    .map(move |(stream, _)| {
                        self.current_step = self
                            .current_step
                            .next()
                            .expect("Extra handshake step taken");
                        (stream, self)
                    }),
            )
        } else {
            let inner = self.inner.take().unwrap();

            Either::B(
                inner
                    .write_handshake_msg(stream, &[])
                    .map(move |(stream, inner)| {
                        self.inner = Some(inner);
                        self.current_step = self
                            .current_step
                            .next()
                            .expect("Extra handshake step taken");
                        (stream, self)
                    }),
            )
        }
    }
}

impl Handshake for NoiseErrorHandshake {
    fn listen<S>(self, stream: S) -> HandshakeResult<S>
    where
        S: AsyncRead + AsyncWrite + 'static,
    {
        let framed = self
            .read_handshake_msg(stream)
            .and_then(|(stream, handshake)| handshake.write_handshake_msg(stream))
            .and_then(|(stream, handshake)| handshake.read_handshake_msg(stream))
            .and_then(|(stream, handshake)| handshake.inner.unwrap().finalize(stream, Vec::new()));
        Box::new(framed)
    }

    fn send<S>(self, stream: S) -> HandshakeResult<S>
    where
        S: AsyncRead + AsyncWrite + 'static,
    {
        let framed = self
            .write_handshake_msg(stream)
            .and_then(|(stream, handshake)| handshake.read_handshake_msg(stream))
            .and_then(|(stream, handshake)| handshake.write_handshake_msg(stream))
            .and_then(|(stream, handshake)| handshake.inner.unwrap().finalize(stream, Vec::new()));
        Box::new(framed)
    }
}
