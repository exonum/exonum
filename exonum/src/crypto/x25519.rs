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

//! X25519 related types and methods used in Diffie-Hellman key exchange.

use sodiumoxide::crypto::sign::ed25519::{convert_ed_keypair_to_curve25519,
                                         PublicKey as PublicKeySodium,
                                         SecretKey as SecretKeySodium};

use std::fmt;

use crypto::{self, PUBLIC_KEY_LENGTH};

const SECRET_KEY_LENGTH: usize = 32;
const BYTES_IN_DEBUG: usize = 4;

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
/// use exonum::crypto;
/// # crypto::init();
///
/// let (pk, sk) = crypto::gen_keypair();
/// let (public_key, secret_key) = crypto::into_x25519_keypair(pk, sk).unwrap();
/// ```
#[cfg_attr(feature = "cargo-clippy", allow(needless_pass_by_value))]
pub fn into_x25519_keypair(
    pk: crypto::PublicKey,
    sk: crypto::SecretKey,
) -> Option<(PublicKey, SecretKey)> {
    let pk_sod = PublicKeySodium::from_slice(&pk[..])?;
    let sk_sod = SecretKeySodium::from_slice(&sk[..])?;

    let (pk, sk) = convert_ed_keypair_to_curve25519(pk_sod, sk_sod);

    let mut secret_key = [0; SECRET_KEY_LENGTH];
    secret_key.clone_from_slice(&sk.0[..SECRET_KEY_LENGTH]);

    Some((PublicKey(pk.0), SecretKey(secret_key)))
}

macro_rules! implement_x25519_type {
    ($(#[$attr:meta])* struct $name:ident, $size:expr) => (
    #[derive(Clone, Copy)]
    $(#[$attr])*
    pub struct $name([u8; $size]);

    impl AsRef<[u8]> for $name {
        fn as_ref(&self) -> &[u8] {
            self.0.as_ref()
        }
    }

    impl fmt::Debug for $name {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, stringify!($name))?;
            write!(f, "(")?;
            for i in &self.as_ref()[0..BYTES_IN_DEBUG] {
                write!(f, "{:02X}", i)?
            }
            write!(f, ")")
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
    struct PublicKey, PUBLIC_KEY_LENGTH
}

implement_x25519_type! {
    /// Curve25519 secret key used in key exchange.
    /// This key cannot be directly generated and can only be converted
    /// from Ed25519 `SecretKey`.
    ///
    /// See: [`into_x25519_keypair()`][1]
    ///
    /// [1]: fn.into_x25519_keypair.html
    struct SecretKey, SECRET_KEY_LENGTH
}
