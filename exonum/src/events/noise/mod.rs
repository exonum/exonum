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
use events::codec::MessagesCodec;
use futures::future::Future;
use std::io;
use tokio_io::{
    AsyncRead,
    AsyncWrite,
    codec::Framed,
    io::{read_exact, write_all},
};

pub mod error;
pub mod wrappers;

#[cfg(test)]
mod tests;

#[cfg(feature = "sodiumoxide-crypto")]
#[doc(inline)]
pub use self::wrappers::sodium_wrapper::{
    wrapper::{
        HANDSHAKE_HEADER_LENGTH,
        NOISE_MAX_HANDSHAKE_MESSAGE_LENGTH,
        NOISE_MIN_HANDSHAKE_MESSAGE_LENGTH,
        NoiseWrapper,
    },
    handshake::{HandshakeParams, NoiseHandshake},
};

pub const NOISE_MAX_MESSAGE_LENGTH: usize = 65_535;
pub const TAG_LENGTH: usize = 16;
pub const NOISE_HEADER_LENGTH: usize = 4;

type HandshakeResult<S> = Box<dyn Future<Item = Framed<S, MessagesCodec>, Error = io::Error>>;

pub trait Handshake {
    fn listen<S: AsyncRead + AsyncWrite + 'static>(self, stream: S) -> HandshakeResult<S>;
    fn send<S: AsyncRead + AsyncWrite + 'static>(self, stream: S) -> HandshakeResult<S>;
}

fn read<S: AsyncRead + 'static>(sock: S) -> impl Future<Item = (S, Vec<u8>), Error = io::Error> {
    let buf = vec![0u8; HANDSHAKE_HEADER_LENGTH];
    // First `HANDSHAKE_HEADER_LENGTH` bytes of handshake message is the payload length
    // in little-endian, remaining bytes is the handshake payload. Therefore, we need to read
    // `HANDSHAKE_HEADER_LENGTH` bytes as a little-endian integer and than we need to read
    // remaining payload.
    read_exact(sock, buf)
        .and_then(|(stream, msg)| {
            let len = LittleEndian::read_uint(&*msg, HANDSHAKE_HEADER_LENGTH);
            read_exact(stream, vec![0u8; len as usize])
        })
}

fn write<S: AsyncWrite + 'static>(
    sock: S,
    buf: &[u8],
    len: usize,
) -> impl Future<Item = (S, Vec<u8>), Error = io::Error> {
    debug_assert!(len < NOISE_MAX_HANDSHAKE_MESSAGE_LENGTH);

    // First `HANDSHAKE_HEADER_LENGTH` bytes of handshake message
    // is the payload length in little-endian.
    let mut message = vec![0u8; HANDSHAKE_HEADER_LENGTH];
    LittleEndian::write_uint(&mut *message, len as u64, HANDSHAKE_HEADER_LENGTH);
    message.extend_from_slice(&buf[0..len]);
    write_all(sock, message)
}
