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

//! Cryptography related types, constants, traits and functions. The functions
//! in this library are used for key generation, hashing, signing and signature
//! verification.
//!
//! The Crypto library makes it possible to potentially change the type of
//! cryptography applied in the system and add abstractions best
//! suited for Exonum.

#[macro_use]
extern crate serde_derive; // Required for Protobuf.

#[doc(inline)]
pub use self::crypto_impl::{
    HASH_SIZE, PUBLIC_KEY_LENGTH, SECRET_KEY_LENGTH, SEED_LENGTH, SIGNATURE_LENGTH,
};
#[cfg(feature = "sodiumoxide-crypto")]
pub use self::crypto_lib::sodiumoxide::x25519;
pub use self::proto::*;

#[cfg(feature = "with-protobuf")]
pub mod proto;

use hex::{encode as encode_hex, FromHex, FromHexError, ToHex};
use serde::{
    de::{self, Deserialize, Deserializer, Visitor},
    Serialize, Serializer,
};

use std::{
    default::Default,
    fmt,
    ops::{Index, Range, RangeFrom, RangeFull, RangeTo},
};

// A way to set an active cryptographic backend is to export it as `crypto_impl`.
#[cfg(feature = "sodiumoxide-crypto")]
use self::crypto_lib::sodiumoxide as crypto_impl;

#[macro_use]
mod macros;

pub(crate) mod crypto_lib;

/// The size to crop the string in debug messages.
const BYTES_IN_DEBUG: usize = 4;

fn write_short_hex(f: &mut fmt::Formatter<'_>, slice: &[u8]) -> fmt::Result {
    for byte in slice.iter().take(BYTES_IN_DEBUG) {
        write!(f, "{:02x}", byte)?;
    }
    if slice.len() > BYTES_IN_DEBUG {
        write!(f, "...")?;
    }
    Ok(())
}

/// Signs a slice of bytes using the signer's secret key and returns the
/// resulting `Signature`.
///
/// # Examples
///
/// The example below generates a pair of secret and public keys, indicates
/// certain data, signs the data using the secret key and with the help of
/// the public key verifies that the data have been signed with the corresponding
/// secret key.
///
/// ```
/// # extern crate exonum_crypto;
///
/// # exonum_crypto::init();
/// let (public_key, secret_key) = exonum_crypto::gen_keypair();
/// let data = [1, 2, 3];
/// let signature = exonum_crypto::sign(&data, &secret_key);
/// assert!(exonum_crypto::verify(&signature, &data, &public_key));
/// ```
pub fn sign(data: &[u8], secret_key: &SecretKey) -> Signature {
    let impl_signature = crypto_impl::sign(data, &secret_key.0);
    Signature(impl_signature)
}

/// Computes a secret key and a corresponding public key from a `Seed`.
///
/// # Examples
///
/// The example below generates a keypair that depends on the indicated seed.
/// Indicating the same seed value always results in the same keypair.
///
/// ```
/// # extern crate exonum_crypto;
/// use exonum_crypto::{SEED_LENGTH, Seed};
///
/// # exonum_crypto::init();
/// let (public_key, secret_key) = exonum_crypto::gen_keypair_from_seed(&Seed::new([1; SEED_LENGTH]));
/// ```
pub fn gen_keypair_from_seed(seed: &Seed) -> (PublicKey, SecretKey) {
    let (impl_pub_key, impl_secret_key) = crypto_impl::gen_keypair_from_seed(&seed.0);
    (PublicKey(impl_pub_key), SecretKey(impl_secret_key))
}

/// Generates a secret key and a corresponding public key using a cryptographically secure
/// pseudo-random number generator.
///
/// # Examples
///
/// The example below generates a unique keypair.
///
/// ```
/// # extern crate exonum_crypto;
///
/// # exonum_crypto::init();
/// let (public_key, secret_key) = exonum_crypto::gen_keypair();
/// ```
pub fn gen_keypair() -> (PublicKey, SecretKey) {
    let (pubkey, secret_key) = crypto_impl::gen_keypair();
    (PublicKey(pubkey), SecretKey(secret_key))
}

/// Verifies that `data` is signed with a secret key corresponding to the
/// given public key.
///
/// # Examples
///
/// The example below generates a pair of secret and public keys, indicates
/// certain data, signs the data using the secret key and with the help of the public key
/// verifies that the data have been signed with the corresponding secret key.
///
/// ```
/// # extern crate exonum_crypto;
///
/// # exonum_crypto::init();
/// let (public_key, secret_key) = exonum_crypto::gen_keypair();
/// let data = [1, 2, 3];
/// let signature = exonum_crypto::sign(&data, &secret_key);
/// assert!(exonum_crypto::verify(&signature, &data, &public_key));
/// ```
pub fn verify(sig: &Signature, data: &[u8], pubkey: &PublicKey) -> bool {
    crypto_impl::verify(&sig.0, data, &pubkey.0)
}

/// Calculates a hash of a bytes slice.
///
/// Type of a hash depends on a chosen crypto backend (via `...-crypto` cargo feature).
///
/// # Examples
///
/// The example below calculates the hash of the indicated data.
///
/// ```
/// # extern crate exonum_crypto;
///
/// # exonum_crypto::init();
/// let data = [1, 2, 3];
/// let hash = exonum_crypto::hash(&data);
/// ```
pub fn hash(data: &[u8]) -> Hash {
    let dig = crypto_impl::hash(data);
    Hash(dig)
}

/// Initializes the cryptographic backend.
///
/// # Panics
///
/// Panics if backend initialization is failed.
///
/// # Examples
///
/// ```
/// # extern crate exonum_crypto;
///
/// exonum_crypto::init();
/// ```
pub fn init() {
    if !crypto_impl::init() {
        panic!("Cryptographic library initialization failed.");
    }
}

/// This structure provides a possibility to calculate a hash digest
/// for a stream of data. Unlike the
/// [`Hash` structure](struct.Hash.html),
/// the given structure lets the code process several data chunks without
/// the need to copy them into a single buffer.
///
/// # Examples
///
/// The example below indicates the data the code is working with; runs the
/// system hash update as many times as required to process all the data chunks
/// and calculates the resulting hash of the system.
///
/// ```rust
/// # extern crate exonum_crypto;
/// use exonum_crypto::HashStream;
///
/// let data: Vec<[u8; 5]> = vec![[1, 2, 3, 4, 5], [6, 7, 8, 9, 10]];
/// let mut hash_stream = HashStream::new();
/// for chunk in data {
///     hash_stream = hash_stream.update(&chunk);
/// }
/// let _ = hash_stream.hash();
/// ```
#[derive(Debug, Default)]
pub struct HashStream(crypto_impl::HashState);

impl HashStream {
    /// Creates a new instance of `HashStream`.
    pub fn new() -> Self {
        HashStream(crypto_impl::HashState::init())
    }

    /// Processes a chunk of stream and returns a `HashStream` with the updated internal state.
    pub fn update(mut self, chunk: &[u8]) -> Self {
        self.0.update(chunk);
        self
    }

    /// Returns the resulting hash of the system calculated upon the commit
    /// of currently supplied data.
    pub fn hash(self) -> Hash {
        let dig = self.0.finalize();
        Hash(dig)
    }
}

/// This structure provides a possibility to create and/or verify
/// digital signatures for a stream of data. If the data are split into several
/// chunks, the indicated chunks are added to the system and when adding is
/// complete, the data is signed.
///
/// # Examples
///
/// The example below adds several data chunks to the system, generates a pair
/// of random public and secret keys, signs the data and verifies the signature.
///
/// ```rust
/// # extern crate exonum_crypto;
/// use exonum_crypto::{SignStream, gen_keypair};
///
/// let data: Vec<[u8; 5]> = vec![[1, 2, 3, 4, 5], [6, 7, 8, 9, 10]];
/// let (public_key, secret_key) = gen_keypair();
/// let mut create_stream = SignStream::new();
/// let mut verify_stream = SignStream::new();
/// for chunk in data {
///     create_stream = create_stream.update(&chunk);
///     verify_stream = verify_stream.update(&chunk);
/// }
/// let file_sign = create_stream.sign(&secret_key);
/// assert!(verify_stream.verify(&file_sign, &public_key));
/// ```
#[derive(Debug, Default)]
pub struct SignStream(crypto_impl::SignState);

impl SignStream {
    /// Creates a new instance of `SignStream`.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate exonum_crypto;
    /// use exonum_crypto::SignStream;
    ///
    /// let stream = SignStream::new();
    /// ```
    pub fn new() -> Self {
        SignStream(crypto_impl::SignState::init())
    }

    /// Adds a new `chunk` to the message that will eventually be signed and/or verified.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate exonum_crypto;
    /// use exonum_crypto::SignStream;
    ///
    /// let mut stream = SignStream::new();
    ///
    /// let data = &[[1, 2, 3], [4, 5, 6], [7, 8, 9]];
    /// for chunk in data.iter() {
    ///     stream = stream.update(chunk);
    /// }
    /// ```
    pub fn update(mut self, chunk: &[u8]) -> Self {
        self.0.update(chunk);
        self
    }

    /// Computes and returns a signature for the previously supplied message
    /// using the given `secret_key`.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate exonum_crypto;
    /// use exonum_crypto::{SignStream, gen_keypair};
    ///
    /// let mut stream = SignStream::new();
    ///
    /// let data = &[[1, 2, 3], [4, 5, 6], [7, 8, 9]];
    /// for chunk in data.iter() {
    ///     stream = stream.update(chunk);
    /// }
    ///
    /// let (public_key, secret_key) = gen_keypair();
    /// let signature = stream.sign(&secret_key);
    /// ```
    pub fn sign(&mut self, secret_key: &SecretKey) -> Signature {
        Signature(self.0.finalize(&secret_key.0))
    }

    /// Verifies that `sig` is a valid signature for the previously supplied message
    /// using the given `public_key`.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate exonum_crypto;
    /// use exonum_crypto::{SignStream, gen_keypair};
    ///
    /// let mut stream = SignStream::new();
    /// let mut verify_stream = SignStream::new();
    ///
    /// let data = &[[1, 2, 3], [4, 5, 6], [7, 8, 9]];
    /// for chunk in data.iter() {
    ///     stream = stream.update(chunk);
    ///     verify_stream = verify_stream.update(chunk);
    /// }
    ///
    /// let (public_key, secret_key) = gen_keypair();
    /// let signature = stream.sign(&secret_key);
    /// assert!(verify_stream.verify(&signature, &public_key));
    /// ```
    pub fn verify(&mut self, sig: &Signature, public_key: &PublicKey) -> bool {
        self.0.verify(&sig.0, &public_key.0)
    }
}

implement_public_crypto_wrapper! {
/// Ed25519 public key used to verify digital signatures.
///
/// In public-key cryptography, the system uses a a mathematically related pair
/// of keys: a public key, which is openly distributed, and a secret key,
/// which should remain confidential. For more information, refer to
/// [Public-key cryptography](https://en.wikipedia.org/wiki/Public-key_cryptography).
///
/// Ed25519 is a signature system that ensures fast signing and key generation,
/// as well as security and collision resilience.
///
/// # Examples
///
/// In the example below, the function generates a pair of random public and
/// secret keys.
///
/// ```
/// # extern crate exonum_crypto;
///
/// # exonum_crypto::init();
/// let (public_key, _) = exonum_crypto::gen_keypair();
/// ```
    struct PublicKey, PUBLIC_KEY_LENGTH
}

implement_private_crypto_wrapper! {
/// Ed25519 secret key used to create digital signatures over messages.
///
/// In public-key cryptography, the system uses a a mathematically related pair
/// of keys: a public key, which is openly distributed, and a secret key,
/// which should remain confidential. For more information, refer to
/// [Public-key cryptography](https://en.wikipedia.org/wiki/Public-key_cryptography).
///
/// Ed25519 is a signature system that ensures fast signing and key generation,
/// as well as security and collision resilience.
///
/// # Examples
///
/// In the example below, the function generates a pair of random public and
/// secret keys.
///
/// ```
/// # extern crate exonum_crypto;
///
/// # exonum_crypto::init();
/// let (_, secret_key) = exonum_crypto::gen_keypair();
/// ```
    struct SecretKey, SECRET_KEY_LENGTH
}

implement_public_crypto_wrapper! {
/// The result of applying the SHA-256 hash function to data.
///
/// This function splits the input data into blocks and runs each block
/// through a cycle of 64 iterations. The result of the function is a hash
/// 256 bits or 32 bytes in length.
///
/// # Examples
///
/// The example below generates the hash of the indicated data.
///
/// ```
/// # extern crate exonum_crypto;
/// use exonum_crypto::Hash;
///
/// let data = [1, 2, 3];
/// let hash_from_data = exonum_crypto::hash(&data);
/// let default_hash = Hash::default();
/// ```
    struct Hash, HASH_SIZE
}

implement_public_crypto_wrapper! {
/// Ed25519 digital signature. This structure creates a signature over data
/// using a secret key. Later it is possible to verify, using the corresponding
/// public key, that the data have indeed been signed with that secret key.
///
/// Ed25519 is a signature system that ensures fast signing and key generation,
/// as well as security and collision resilience.
///
/// # Examples
///
/// The example below generates a pair of random public and secret keys,
/// adds certain data, signs the data using the secret key and verifies
/// that the data have been signed with that secret key.
///
/// ```
/// # extern crate exonum_crypto;
///
/// # exonum_crypto::init();
/// let (public_key, secret_key) = exonum_crypto::gen_keypair();
/// let data = [1, 2, 3];
/// let signature = exonum_crypto::sign(&data, &secret_key);
/// assert!(exonum_crypto::verify(&signature, &data, &public_key));
/// ```
    struct Signature, SIGNATURE_LENGTH
}

implement_private_crypto_wrapper! {
/// Ed25519 seed representing a succession of bytes that can be used for
/// deterministic keypair generation. If the same seed is indicated in the
/// generator multiple times, the generated keys will be the same each time.
///
/// Note that this is not the seed added to Exonum transactions for additional
/// security, this is a separate entity. This structure is useful for testing,
/// to receive repeatable results. The seed in this structure is either set
/// manually or selected using the methods below.
///
/// # Examples
///
/// The example below generates a pair of public and secret keys taking
/// into account the selected seed. The same seed will always lead to
/// generation of the same keypair.
///
/// ```
/// # extern crate exonum_crypto;
/// use exonum_crypto::{SEED_LENGTH, Seed};
///
/// # exonum_crypto::init();
/// let (public_key, secret_key) = exonum_crypto::gen_keypair_from_seed(&Seed::new([1; SEED_LENGTH]));
/// ```
    struct Seed, SEED_LENGTH
}

implement_serde! {Hash}
implement_serde! {PublicKey}
implement_serde! {SecretKey}
implement_serde! {Seed}
implement_serde! {Signature}

implement_index_traits! {Hash}
implement_index_traits! {PublicKey}
implement_index_traits! {SecretKey}
implement_index_traits! {Seed}
implement_index_traits! {Signature}

#[derive(Debug, Default, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct KeyPair {
    public_key: PublicKey,
    secret_key: SecretKey,
}

impl KeyPair {
    pub fn from_keys(public_key: PublicKey, secret_key: SecretKey) -> Self {
        debug_assert!(
            verify_keys_match(&public_key, &secret_key),
            "Public key does not match the secret key."
        );

        Self {
            public_key,
            secret_key,
        }
    }

    pub fn public_key(&self) -> PublicKey {
        self.public_key
    }

    pub fn secret_key(&self) -> &SecretKey {
        &self.secret_key
    }
}

impl From<(PublicKey, SecretKey)> for KeyPair {
    fn from(keys: (PublicKey, SecretKey)) -> Self {
        Self::from_keys(keys.0, keys.1)
    }
}

fn verify_keys_match(public_key: &PublicKey, secret_key: &SecretKey) -> bool {
    crypto_impl::verify_keys_match(&public_key.0, &secret_key.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    use serde::de::DeserializeOwned;

    use hex::FromHex;

    #[test]
    fn to_from_hex_hash() {
        let original = hash(&[]);
        let from_hex = Hash::from_hex(original.to_hex()).unwrap();
        assert_eq!(original, from_hex);
    }

    #[test]
    fn zero_hash() {
        let hash = Hash::zero();
        assert_eq!(hash.as_ref(), [0; HASH_SIZE]);
    }

    #[test]
    fn to_from_hex_keys() {
        let (p, s) = gen_keypair();

        let ph = PublicKey::from_hex(p.to_hex()).unwrap();
        assert_eq!(p, ph);

        let sh = SecretKey::from_hex(s.to_hex()).unwrap();
        assert_eq!(s, sh);
    }

    #[test]
    fn serialize_deserialize_hash() {
        assert_serialize_deserialize(&Hash::new([207; HASH_SIZE]));
    }

    #[test]
    fn serialize_deserialize_public_key() {
        assert_serialize_deserialize(&PublicKey::new([208; PUBLIC_KEY_LENGTH]));
    }

    #[test]
    fn serialize_deserialize_signature() {
        assert_serialize_deserialize(&Signature::new([209; SIGNATURE_LENGTH]));
    }

    #[test]
    fn serialize_deserialize_seed() {
        assert_serialize_deserialize(&Seed::new([210; SEED_LENGTH]));
    }

    #[test]
    fn serialize_deserialize_secret_key() {
        assert_serialize_deserialize(&SecretKey::new([211; SECRET_KEY_LENGTH]));
    }

    #[test]
    fn debug_format() {
        // Check zero padding.
        let hash = Hash::new([1; HASH_SIZE]);
        assert_eq!(format!("{:?}", &hash), "Hash(01010101...)");

        let pk = PublicKey::new([15; PUBLIC_KEY_LENGTH]);
        assert_eq!(format!("{:?}", &pk), "PublicKey(0f0f0f0f...)");
        let sk = SecretKey::new([8; SECRET_KEY_LENGTH]);
        assert_eq!(format!("{:?}", &sk), "SecretKey(08080808...)");
        let signature = Signature::new([10; SIGNATURE_LENGTH]);
        assert_eq!(format!("{:?}", &signature), "Signature(0a0a0a0a...)");
        let seed = Seed::new([4; SEED_LENGTH]);
        assert_eq!(format!("{:?}", &seed), "Seed(04040404...)");

        // Check no padding.
        let hash = Hash::new([128; HASH_SIZE]);
        assert_eq!(format!("{:?}", &hash), "Hash(80808080...)");
        let sk = SecretKey::new([255; SECRET_KEY_LENGTH]);
        assert_eq!(format!("{:?}", &sk), "SecretKey(ffffffff...)");
    }

    // Note that only public values have Display impl.
    #[test]
    fn display_format() {
        // Check zero padding.
        let hash = Hash::new([1; HASH_SIZE]);
        assert_eq!(format!("{}", &hash), "01010101...");

        let pk = PublicKey::new([15; PUBLIC_KEY_LENGTH]);
        assert_eq!(format!("{}", &pk), "0f0f0f0f...");
        let signature = Signature::new([10; SIGNATURE_LENGTH]);
        assert_eq!(format!("{}", &signature), "0a0a0a0a...");

        // Check no padding.
        let hash = Hash::new([128; HASH_SIZE]);
        assert_eq!(format!("{}", &hash), "80808080...");
    }

    #[test]
    fn hash_streaming_zero() {
        let h1 = hash(&[]);
        let state = HashStream::new();
        let h2 = state.update(&[]).hash();
        assert_eq!(h1, h2);
    }

    #[test]
    fn hash_streaming_chunks() {
        let data: [u8; 10] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 0];
        let h1 = hash(&data);
        let state = HashStream::new();
        let h2 = state.update(&data[..5]).update(&data[5..]).hash();
        assert_eq!(h1, h2);
    }

    #[test]
    fn sign_streaming_zero() {
        let (pk, sk) = gen_keypair();
        let mut creation_stream = SignStream::new().update(&[]);
        let sig = creation_stream.sign(&sk);
        let mut verified_stream = SignStream::new().update(&[]);
        assert!(verified_stream.verify(&sig, &pk));
    }

    #[test]
    fn sign_streaming_chunks() {
        let data: [u8; 10] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 0];
        let (pk, sk) = gen_keypair();
        let mut creation_stream = SignStream::new().update(&data[..5]).update(&data[5..]);
        let sig = creation_stream.sign(&sk);
        let mut verified_stream = SignStream::new().update(&data[..5]).update(&data[5..]);
        assert!(verified_stream.verify(&sig, &pk));
    }

    fn assert_serialize_deserialize<T>(original_value: &T)
    where
        T: Serialize + DeserializeOwned + PartialEq + fmt::Debug,
    {
        let json = serde_json::to_string(original_value).unwrap();
        let deserialized_value: T = serde_json::from_str(&json).unwrap();
        assert_eq!(*original_value, deserialized_value);
    }

    #[test]
    fn valid_keypair() {
        let (pk, sk) = gen_keypair();
        let _ = KeyPair::from_keys(pk, sk);
    }

    #[test]
    #[should_panic]
    fn not_valid_keypair() {
        let (pk, _) = gen_keypair();
        let (_, sk) = gen_keypair();
        let _ = KeyPair::from_keys(pk, sk);
    }
}
