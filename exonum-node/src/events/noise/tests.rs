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

use async_trait::async_trait;
use byteorder::{ByteOrder, LittleEndian};
use bytes::BytesMut;
use exonum::{
    crypto::{gen_keypair_from_seed, Seed, PUBLIC_KEY_LENGTH, SEED_LENGTH},
    merkledb::BinaryValue,
};
use futures::{channel::mpsc, prelude::*};
use pretty_assertions::assert_eq;
use snow::{types::Dh, Builder};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::{TcpListener, TcpStream},
    time::delay_for,
};

use std::{net::SocketAddr, time::Duration};

use crate::events::noise::HandshakeData;
use crate::events::{
    noise::{
        wrappers::sodium_wrapper::resolver::{SodiumDh25519, SodiumResolver},
        Handshake, HandshakeParams, HandshakeRawMessage, NoiseHandshake, NoiseWrapper,
        TransportWrapper, HEADER_LENGTH, MAX_MESSAGE_LENGTH,
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
    let mut keypair_i = SodiumDh25519::default();
    keypair_i.set(secret_key_i.as_ref());
    let mut output_i = [0_u8; PUBLIC_KEY_LENGTH];
    keypair_i.dh(public_key_r.as_ref(), &mut output_i).unwrap();

    let mut keypair_r = SodiumDh25519::default();
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

    let mut initiator = Builder::with_resolver(PATTERN.parse().unwrap(), Box::new(SodiumResolver))
        .local_private_key(secret_key_i.as_ref())
        .remote_public_key(public_key_r.as_ref())
        .build_initiator()
        .expect("Unable to create initiator");

    let mut responder = Builder::with_resolver(PATTERN.parse().unwrap(), Box::new(SodiumResolver))
        .local_private_key(secret_key_r.as_ref())
        .build_responder()
        .expect("Unable to create responder");

    let mut buffer_msg = [0_u8; MSG_SIZE * 2];
    let mut buffer_out = [0_u8; MSG_SIZE * 2];

    let len = initiator
        .write_message(&[0_u8; 0], &mut buffer_msg)
        .unwrap();
    responder
        .read_message(&buffer_msg[..len], &mut buffer_out)
        .unwrap();
    let second_len = responder
        .write_message(&[0_u8; 0], &mut buffer_msg)
        .unwrap();
    initiator
        .read_message(&buffer_msg[..second_len], &mut buffer_out)
        .unwrap();
    let third_len = initiator
        .write_message(&[0_u8; 0], &mut buffer_msg)
        .unwrap();
    responder
        .read_message(&buffer_msg[..third_len], &mut buffer_out)
        .unwrap();

    responder
        .into_transport_mode()
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
    let mut bytes = BytesMut::with_capacity(len + HEADER_LENGTH);
    bytes.resize(len + HEADER_LENGTH, 0);
    let res = responder.decrypt_msg(len, &mut bytes);
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
        Self { step, message }
    }
}

#[derive(Debug, PartialEq, Copy, Clone)]
enum HandshakeStep {
    EphemeralKeyExchange,
    StaticKeyExchange,
    Done,
}

impl HandshakeStep {
    fn next(self) -> Option<Self> {
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

#[tokio::test]
async fn test_noise_handshake_errors_ee_empty() {
    let addr: SocketAddr = "127.0.0.1:45003".parse().unwrap();
    let params = HandshakeParams::with_default_params();
    let bogus_message = Some(BogusMessage::new(
        HandshakeStep::EphemeralKeyExchange,
        EMPTY_MESSAGE,
    ));
    let (_, listener_err) = wait_for_handshake_result(addr, params, bogus_message, None).await;
    let listener_err = listener_err.to_string();
    assert!(
        listener_err.contains("Wrong handshake message length"),
        "{}",
        listener_err
    );
}

#[tokio::test]
async fn test_noise_handshake_errors_es_empty() {
    let addr: SocketAddr = "127.0.0.1:45004".parse().unwrap();
    let params = HandshakeParams::with_default_params();
    let bogus_message = Some(BogusMessage::new(
        HandshakeStep::StaticKeyExchange,
        EMPTY_MESSAGE,
    ));
    let (_, listener_err) = wait_for_handshake_result(addr, params, bogus_message, None).await;
    let listener_err = listener_err.to_string();
    assert!(
        listener_err.contains("Wrong handshake message length"),
        "{}",
        listener_err
    );
}

#[tokio::test]
async fn test_noise_handshake_errors_ee_standard() {
    let addr: SocketAddr = "127.0.0.1:45005".parse().unwrap();
    let params = HandshakeParams::with_default_params();
    let bogus_message = Some(BogusMessage::new(
        HandshakeStep::EphemeralKeyExchange,
        STANDARD_MESSAGE,
    ));
    let (_, listener_err) = wait_for_handshake_result(addr, params, bogus_message, None).await;
    let listener_err = listener_err.to_string();
    assert!(
        listener_err.contains("diffie-hellman error"),
        "{}",
        listener_err
    );
}

#[tokio::test]
async fn test_noise_handshake_errors_es_standard() {
    let addr: SocketAddr = "127.0.0.1:45006".parse().unwrap();
    let params = HandshakeParams::with_default_params();
    let bogus_message = Some(BogusMessage::new(
        HandshakeStep::StaticKeyExchange,
        STANDARD_MESSAGE,
    ));

    let (_, listener_err) = wait_for_handshake_result(addr, params, bogus_message, None).await;
    let listener_err = listener_err.to_string();
    assert!(listener_err.contains("decrypt error"), "{}", listener_err);
}

#[tokio::test]
async fn test_noise_handshake_errors_ee_empty_listen() {
    let addr: SocketAddr = "127.0.0.1:45007".parse().unwrap();
    let params = HandshakeParams::with_default_params();
    let bogus_message = Some(BogusMessage::new(
        HandshakeStep::EphemeralKeyExchange,
        EMPTY_MESSAGE,
    ));
    let (sender_res, _) = wait_for_handshake_result(addr, params, None, bogus_message).await;
    let sender_err = sender_res.unwrap_err().to_string();
    assert!(
        sender_err.contains("Wrong handshake message length"),
        "{}",
        sender_err
    );
}

#[tokio::test]
async fn test_noise_handshake_errors_ee_standard_listen() {
    let addr: SocketAddr = "127.0.0.1:45008".parse().unwrap();
    let params = HandshakeParams::with_default_params();
    let bogus_message = Some(BogusMessage::new(
        HandshakeStep::EphemeralKeyExchange,
        STANDARD_MESSAGE,
    ));
    let (sender_res, _) = wait_for_handshake_result(addr, params, None, bogus_message).await;
    let sender_err = sender_res.unwrap_err().to_string();
    assert!(
        sender_err.contains("diffie-hellman error"),
        "{}",
        sender_err
    );
}

#[tokio::test]
async fn test_noise_handshake_wrong_remote_key() {
    let addr: SocketAddr = "127.0.0.1:45009".parse().unwrap();
    let mut params = HandshakeParams::with_default_params();
    let (remote_key, _) = gen_keypair_from_seed(&Seed::new([2; SEED_LENGTH]));
    params.set_remote_key(remote_key);

    let (_, listener_err) = wait_for_handshake_result(addr, params, None, None).await;
    let listener_err = listener_err.to_string();
    assert!(listener_err.contains("decrypt error"), "{}", listener_err);
}

// We need check result from both: sender and responder.
async fn wait_for_handshake_result(
    addr: SocketAddr,
    params: HandshakeParams,
    sender_message: Option<BogusMessage>,
    responder_message: Option<BogusMessage>,
) -> (anyhow::Result<()>, anyhow::Error) {
    let (err_tx, mut err_rx) = mpsc::channel(1);
    tokio::spawn(run_handshake_listener(
        addr,
        params.clone(),
        err_tx,
        responder_message,
    ));
    delay_for(Duration::from_millis(500)).await;

    let sender_err = send_handshake(addr, params, sender_message).await;
    let listener_err = err_rx.next().await.expect("No listener error sent");
    (sender_err, listener_err)
}

async fn run_handshake_listener(
    addr: SocketAddr,
    params: HandshakeParams,
    err_sender: mpsc::Sender<anyhow::Error>,
    bogus_message: Option<BogusMessage>,
) -> anyhow::Result<()> {
    let mut listener = TcpListener::bind(addr).await?;
    let mut incoming_connections = listener.incoming();

    while let Some(mut stream) = incoming_connections.try_next().await? {
        let mut err_sender = err_sender.clone();
        let params = params.clone();
        tokio::spawn(async move {
            let response = if let Some(message) = bogus_message {
                NoiseErrorHandshake::responder(&params, message).listen(&mut stream)
            } else {
                NoiseHandshake::responder(&params).listen(&mut stream)
            };

            if let Err(err) = response.await {
                err_sender.send(err).await.ok();
            }
        });
    }
    Ok(())
}

async fn send_handshake(
    addr: SocketAddr,
    params: HandshakeParams,
    bogus_message: Option<BogusMessage>,
) -> anyhow::Result<()> {
    let mut stream = TcpStream::connect(addr).await?;
    if let Some(message) = bogus_message {
        NoiseErrorHandshake::initiator(&params, message)
            .send(&mut stream)
            .await
            .map(drop)
    } else {
        NoiseHandshake::initiator(&params)
            .send(&mut stream)
            .await
            .map(drop)
    }
}

#[derive(Debug)]
struct NoiseErrorHandshake {
    bogus_message: BogusMessage,
    current_step: HandshakeStep,
    // Option is used in order to be able to move out `inner` from the instance.
    inner: NoiseHandshake,
}

impl NoiseErrorHandshake {
    fn initiator(params: &HandshakeParams, bogus_message: BogusMessage) -> Self {
        Self {
            bogus_message,
            current_step: HandshakeStep::EphemeralKeyExchange,
            inner: NoiseHandshake::initiator(params),
        }
    }

    fn responder(params: &HandshakeParams, bogus_message: BogusMessage) -> Self {
        Self {
            bogus_message,
            current_step: HandshakeStep::EphemeralKeyExchange,
            inner: NoiseHandshake::responder(params),
        }
    }

    async fn read_handshake_msg<S>(&mut self, stream: &mut S) -> anyhow::Result<()>
    where
        S: AsyncRead + Unpin,
    {
        self.inner.read_handshake_msg(stream).await.map(drop)
    }

    async fn write_handshake_msg<S>(&mut self, stream: &mut S) -> anyhow::Result<()>
    where
        S: AsyncWrite + Unpin,
    {
        if self.current_step == self.bogus_message.step {
            let msg = self.bogus_message.message;
            HandshakeRawMessage(msg.to_vec()).write(stream).await?;
        } else {
            self.inner.write_handshake_msg(stream, &[]).await?;
        }

        self.current_step = self
            .current_step
            .next()
            .expect("Extra handshake step taken");
        Ok(())
    }
}

#[async_trait]
impl<S> Handshake<S> for NoiseErrorHandshake
where
    S: AsyncRead + AsyncWrite + 'static + Send + Unpin,
{
    async fn listen(mut self, stream: &mut S) -> anyhow::Result<HandshakeData> {
        self.read_handshake_msg(stream).await?;
        self.write_handshake_msg(stream).await?;
        self.read_handshake_msg(stream).await?;
        self.inner.finalize(vec![])
    }

    async fn send(mut self, stream: &mut S) -> anyhow::Result<HandshakeData> {
        self.write_handshake_msg(stream).await?;
        self.read_handshake_msg(stream).await?;
        self.write_handshake_msg(stream).await?;
        self.inner.finalize(vec![])
    }
}
