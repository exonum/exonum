// Copyright 2019 The Exonum Team
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

use std::{
    fmt,
    ops::{Index, Range, RangeFrom, RangeFull, RangeTo},
};

use crate::{write_short_hex, Seed};
use exonum_sodiumoxide::crypto::kx;
use hex::{encode as encode_hex, FromHex, FromHexError};

use serde::{
    de::{self, Deserialize, Deserializer, Visitor},
    Serialize, Serializer,
};

pub fn gen_keypair() -> (PublicKey, SecretKey) {
    let (pk, sk) = kx::gen_keypair();

    (PublicKey(pk), SecretKey(sk))
}

pub fn gen_keypair_from_seed(seed: &Seed) -> (PublicKey, SecretKey) {
    let (pk, sk) = kx::keypair_from_seed(&kx::Seed::from_slice(&seed[..]).unwrap());

    (PublicKey(pk), SecretKey(sk))
}

#[derive(Debug, Copy, Hash, Clone, PartialEq, Eq, Ord, PartialOrd)]
pub struct PublicKey(kx::PublicKey);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretKey(kx::SecretKey);

impl PublicKey {
    pub fn from_slice(bytes_slice: &[u8]) -> Option<Self> {
        kx::PublicKey::from_slice(bytes_slice).map(PublicKey)
    }
}

impl fmt::Display for PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write_short_hex(f, &self[..])
    }
}

impl SecretKey {
    pub fn from_slice(bytes_slice: &[u8]) -> Option<Self> {
        kx::SecretKey::from_slice(bytes_slice).map(SecretKey)
    }
}

impl AsRef<[u8]> for PublicKey {
    fn as_ref(&self) -> &[u8] {
        &self.0[..]
    }
}

impl AsRef<[u8]> for SecretKey {
    fn as_ref(&self) -> &[u8] {
        &self.0[..]
    }
}

implement_serde! { PublicKey }
implement_serde! { SecretKey }
implement_index_traits! { PublicKey }
implement_index_traits! { SecretKey }
