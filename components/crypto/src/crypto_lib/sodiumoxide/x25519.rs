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

//! X25519 related types and methods used in Diffie-Hellman key exchange.

use std::{
    fmt,
    ops::{Index, Range, RangeFrom, RangeFull, RangeTo},
};

use super::sodiumoxide::crypto::{
    scalarmult::curve25519::{
        scalarmult as sodium_scalarmult, scalarmult_base as sodium_scalarmult_base,
        GroupElement as Curve25519GroupElement, Scalar as Curve25519Scalar,
    },
    sign::ed25519::{
        convert_ed_keypair_to_curve25519, convert_ed_pk_to_curve25519, convert_ed_sk_to_curve25519,
        PublicKey as PublicKeySodium, SecretKey as SecretKeySodium,
    },
};
use crate::{write_short_hex, PublicKey as crypto_PublicKey, SecretKey as crypto_SecretKey};

/// Length of the public Curve25519 key.
pub const PUBLIC_KEY_LENGTH: usize = 32;
/// Length of the secret Curve25519 key.
pub const SECRET_KEY_LENGTH: usize = 32;

/// Converts Ed25519 keys to Curve25519.
///
/// Ed25519 keys used for signatures can be converted to Curve25519 and used for
/// Diffie-Hellman key exchange.
///
/// # Examples
///
/// The example below generates a pair of secret and public Ed25519 keys and
/// converts it to pair of Curve25519 keys.
///
/// ```
/// # exonum_crypto::init();
///
/// let (pk, sk) = exonum_crypto::gen_keypair();
/// let (public_key, secret_key) = exonum_crypto::x25519::into_x25519_keypair(pk, sk).unwrap();
/// ```
#[cfg_attr(feature = "cargo-clippy", allow(clippy::needless_pass_by_value))]
pub fn into_x25519_keypair(
    pk: crypto_PublicKey,
    sk: crypto_SecretKey,
) -> Option<(PublicKey, SecretKey)> {
    let pk_sod = PublicKeySodium::from_slice(&pk[..])?;
    let sk_sod = SecretKeySodium::from_slice(&sk[..])?;

    let (pk, sk) = convert_ed_keypair_to_curve25519(pk_sod, sk_sod);

    let mut secret_key = [0; SECRET_KEY_LENGTH];
    secret_key.clone_from_slice(&sk.0[..SECRET_KEY_LENGTH]);

    Some((PublicKey::new(pk.0), SecretKey::new(secret_key)))
}

/// Converts an arbitrary array of data to the Curve25519-compatible private key.
pub fn convert_to_private_key(key: &mut [u8; 32]) {
    let converted = convert_ed_sk_to_curve25519(key);

    key.copy_from_slice(&converted);
}

/// Calculates the scalar multiplication for X25519.
pub fn scalarmult(sc: &SecretKey, pk: &PublicKey) -> Result<PublicKey, ()> {
    sodium_scalarmult(sc.as_ref(), pk.as_ref()).map(PublicKey)
}

/// Calculates the public key based on private key for X25519.
pub fn scalarmult_base(sc: &SecretKey) -> PublicKey {
    sodium_scalarmult_base(sc.as_ref()).into()
}

/// Converts Ed25519 public key to Curve25519 public key.
///
/// See also: [`into_x25519_keypair()`](fn.into_x25519_public_key.html)
pub fn into_x25519_public_key(pk: crypto_PublicKey) -> PublicKey {
    let mut public_key = [0; PUBLIC_KEY_LENGTH];
    public_key.clone_from_slice(&pk[..PUBLIC_KEY_LENGTH]);
    let public_key = convert_ed_pk_to_curve25519(&public_key);
    PublicKey(Curve25519GroupElement(public_key))
}

macro_rules! implement_x25519_type {
    ($(#[$attr:meta])* struct $name:ident, $name_from:ident, $size:expr) => (
    #[derive(PartialEq, Eq, Clone)]
    $(#[$attr])*
    pub struct $name($name_from);

    impl $name {
        /// Creates a new instance filled with zeros.
        pub fn zero() -> Self {
            $name::new([0; $size])
        }
    }

    impl $name {
        /// Creates a new instance from bytes array.
        pub fn new(bytes_array: [u8; $size]) -> Self {
            $name($name_from(bytes_array))
        }

        /// Creates a new instance from bytes slice.
        pub fn from_slice(bytes_slice: &[u8]) -> Option<Self> {
            $name_from::from_slice(bytes_slice).map($name)
        }
    }

    impl AsRef<[u8]> for $name {
        fn as_ref(&self) -> &[u8] {
            &self.0[..]
        }
    }

    impl AsRef<$name_from> for $name {
        fn as_ref(&self) -> &$name_from {
            &self.0
        }
    }

    impl fmt::Debug for $name {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, stringify!($name))?;
            write!(f, "(")?;
            write_short_hex(f, &self.0[..])?;
            write!(f, ")")
        }
    }

    impl Into<$name> for $name_from {
        fn into(self) -> $name {
            $name(self)
        }
    }
    )
}

implement_x25519_type! {
    /// Curve25519 public key used in key exchange.
    /// This key cannot be directly generated and can only be converted
    /// from Ed25519 `PublicKey`.
    ///
    /// See: [`into_x25519_keypair()`][1]
    ///
    /// [1]: fn.into_x25519_keypair.html
    struct PublicKey, Curve25519GroupElement, PUBLIC_KEY_LENGTH
}

implement_x25519_type! {
    /// Curve25519 secret key used in key exchange.
    /// This key cannot be directly generated and can only be converted
    /// from Ed25519 `SecretKey`.
    ///
    /// See: [`into_x25519_keypair()`][1]
    ///
    /// [1]: fn.into_x25519_keypair.html
    struct SecretKey, Curve25519Scalar, SECRET_KEY_LENGTH
}

implement_index_traits! { PublicKey }
implement_index_traits! { SecretKey }
