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

//! This module implements cryptographic backend based
//! on [Sodium library](https://github.com/jedisct1/libsodium)
//! through [sodiumoxide rust bindings](https://github.com/dnaq/sodiumoxide).
//! The constants in this module are imported from Sodium.
//!
//! The SHA-256 function applied in this backend splits the input data into blocks
//! and runs each block through a cycle of 64 iterations. The result of the
//! function is a cryptographic hash 256 bits or 32 bytes in length. This
//! hash can later be used to verify the integrity of data without accessing the
//! data itself.
//!
//! This backend also makes use of Ed25519 keys. Ed25519 is a signature system that ensures
//! fast signing and key generation, as well as security and collision
//! resilience.

// spell-checker:ignore DIGESTBYTES, PUBLICKEYBYTES, SECRETKEYBYTES, SEEDBYTES, SIGNATUREBYTES

use exonum_sodiumoxide as sodiumoxide;

/// Digest type for sodiumoxide-based implementation.
pub use self::sha256::Digest as Hash;

/// Signature type for sodiumoxide-based implementation.
pub use self::ed25519::Signature;

/// Secret key type for sodiumoxide-based implementation.
pub use self::ed25519::SecretKey;

/// Public key type for sodiumoxide-based implementation.
pub use self::ed25519::PublicKey;

/// Seed type for sodiumoxide-based implementation.
pub use self::ed25519::Seed;

/// State for multi-part (streaming) computation of signature for sodiumoxide-based
/// implementation.
pub use self::ed25519::State as SignState;

/// Contains the state for multi-part (streaming) hash computations
/// for sodiumoxide-based implementation.
pub use self::sha256::State as HashState;

use self::sodiumoxide::crypto::{
    hash::sha256,
    sign::{convert_sk_to_pk, ed25519},
};

pub mod x25519;

/// Number of bytes in a `Hash`.
pub const HASH_SIZE: usize = sha256::DIGESTBYTES;

/// Number of bytes in a public key.
pub const PUBLIC_KEY_LENGTH: usize = ed25519::PUBLICKEYBYTES;

/// Number of bytes in a secret key.
pub const SECRET_KEY_LENGTH: usize = ed25519::SECRETKEYBYTES;

/// Number of bytes in a seed.
pub const SEED_LENGTH: usize = ed25519::SEEDBYTES;

/// Number of bytes in a signature.
pub const SIGNATURE_LENGTH: usize = ed25519::SIGNATUREBYTES;

/// Initializes the sodium library and automatically selects faster versions
/// of the primitives, if possible.
pub fn init() -> bool {
    sodiumoxide::init()
}

/// Signs a slice of bytes using the signer's secret key and returns the
/// resulting `Signature`.
pub fn sign(data: &[u8], secret_key: &SecretKey) -> Signature {
    ed25519::sign_detached(data, secret_key)
}

/// Computes a secret key and a corresponding public key from a `Seed`.
pub fn gen_keypair_from_seed(seed: &Seed) -> (PublicKey, SecretKey) {
    ed25519::keypair_from_seed(seed)
}

/// Generates a secret key and a corresponding public key using a cryptographically secure
/// pseudo-random number generator.
pub fn gen_keypair() -> (PublicKey, SecretKey) {
    ed25519::gen_keypair()
}

/// Verifies that `data` is signed with a secret key corresponding to the
/// given public key.
pub fn verify(sig: &Signature, data: &[u8], pub_key: &PublicKey) -> bool {
    ed25519::verify_detached(sig, data, pub_key)
}

/// Calculates hash of a bytes slice.
pub fn hash(data: &[u8]) -> Hash {
    sha256::hash(data)
}

/// Verifies that public key matches provided secret key.
pub(crate) fn verify_keys_match(public_key: &PublicKey, secret_key: &SecretKey) -> bool {
    convert_sk_to_pk(&secret_key) == *public_key
}
