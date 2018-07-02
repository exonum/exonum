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

//! Cryptography related types, constants, traits and functions. The functions
//! in this module are used for key generation, hashing, signing and signature
//! verification.
//!
//! The SHA-256 function applied in Exonum splits the input data into blocks
//! and runs each block through a cycle of 64 iterations. The result of the
//! function is a cryptographic hash 256 bits or 32 bytes in length. This
//! hash can later be used to verify the integrity of data without accessing the
//! data itself.
//!
//! Exonum also makes use of Ed25519 keys. Ed25519 is a signature system that ensures
//! fast signing and key generation, as well as security and collision
//! resilience.
//!
//! [Sodium library](https://github.com/jedisct1/libsodium)
//! is used under the hood through [sodiumoxide rust bindings](https://github.com/dnaq/sodiumoxide).
//! The constants in this module are imported from Sodium.
//!
//! The Crypto module makes it possible to potentially change the type of
//! cryptography applied in the system and add abstractions best
//! suited for Exonum.

// spell-checker:disable
pub use sodiumoxide::crypto::{
    hash::sha256::DIGESTBYTES as HASH_SIZE,
    sign::ed25519::{
        PUBLICKEYBYTES as PUBLIC_KEY_LENGTH, SECRETKEYBYTES as SECRET_KEY_LENGTH,
        SEEDBYTES as SEED_LENGTH, SIGNATUREBYTES as SIGNATURE_LENGTH,
    },
};
// spell-checker:enable

use byteorder::{ByteOrder, LittleEndian};
use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use serde::{
    de::{self, Deserialize, Deserializer, Visitor}, Serialize, Serializer,
};
use sodiumoxide::{
    self,
    crypto::{
        hash::sha256::{hash as hash_sodium, Digest as DigestSodium, State as HashState},
        sign::ed25519::{
            gen_keypair as gen_keypair_sodium, keypair_from_seed, sign_detached, verify_detached,
            PublicKey as PublicKeySodium, SecretKey as SecretKeySodium, Seed as SeedSodium,
            Signature as SignatureSodium, State as SignState,
        },
    },
};
use uuid::Uuid;

use std::{
    default::Default, fmt, ops::{Index, Range, RangeFrom, RangeFull, RangeTo}, str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};

use encoding::{
    serialize::{encode_hex, FromHex, FromHexError, ToHex}, Field, Offset,
};
use helpers::Round;

#[macro_use]
mod macros;

pub mod x25519;

/// The size to crop the string in debug messages.
const BYTES_IN_DEBUG: usize = 4;

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
/// use exonum::crypto;
///
/// # crypto::init();
/// let (public_key, secret_key) = crypto::gen_keypair();
/// let data = [1, 2, 3];
/// let signature = crypto::sign(&data, &secret_key);
/// assert!(crypto::verify(&signature, &data, &public_key));
/// ```
pub fn sign(data: &[u8], secret_key: &SecretKey) -> Signature {
    let sodium_signature = sign_detached(data, &secret_key.0);
    Signature(sodium_signature)
}

/// Computes a secret key and a corresponding public key from a `Seed`.
///
/// # Examples
///
/// The example below generates a keypair that depends on the indicated seed.
/// Indicating the same seed value always results in the same keypair.
///
/// ```
/// use exonum::crypto::{self, Seed};
///
/// # crypto::init();
/// let (public_key, secret_key) = crypto::gen_keypair_from_seed(&Seed::new([1; 32]));
/// ```
pub fn gen_keypair_from_seed(seed: &Seed) -> (PublicKey, SecretKey) {
    let (sod_pub_key, sod_secret_key) = keypair_from_seed(&seed.0);
    (PublicKey(sod_pub_key), SecretKey(sod_secret_key))
}

/// Generates a secret key and a corresponding public key using a cryptographically secure
/// pseudo-random number generator.
///
/// # Examples
///
/// The example below generates a unique keypair.
///
/// ```
/// use exonum::crypto;
///
/// # crypto::init();
/// let (public_key, secret_key) = crypto::gen_keypair();
/// ```
pub fn gen_keypair() -> (PublicKey, SecretKey) {
    let (pubkey, secret_key) = gen_keypair_sodium();
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
/// use exonum::crypto;
///
/// # crypto::init();
/// let (public_key, secret_key) = crypto::gen_keypair();
/// let data = [1, 2, 3];
/// let signature = crypto::sign(&data, &secret_key);
/// assert!(crypto::verify(&signature, &data, &public_key));
/// ```
pub fn verify(sig: &Signature, data: &[u8], pubkey: &PublicKey) -> bool {
    verify_detached(&sig.0, data, &pubkey.0)
}

/// Calculates an SHA-256 hash of a bytes slice.
///
/// # Examples
///
/// The example below calculates the hash of the indicated data.
///
/// ```
/// use exonum::crypto;
///
/// # crypto::init();
/// let data = [1, 2, 3];
/// let hash = crypto::hash(&data);
/// ```
pub fn hash(data: &[u8]) -> Hash {
    let dig = hash_sodium(data);
    Hash(dig)
}

/// A common trait for the ability to compute a cryptographic hash.
pub trait CryptoHash {
    /// Returns a hash of the value.
    ///
    /// The hashing strategy must satisfy the basic requirements of cryptographic hashing:
    /// equal values must have the same hash and not equal values must have different hashes
    /// (except for negligible probability).
    fn hash(&self) -> Hash;
}

/// Initializes the sodium library and automatically selects faster versions
/// of the primitives, if possible.
///
/// # Panics
///
/// Panics if sodium initialization is failed.
///
/// # Examples
///
/// ```
/// use exonum::crypto;
///
/// crypto::init();
/// ```
pub fn init() {
    if !sodiumoxide::init() {
        panic!("Cryptographic library hasn't initialized.");
    }
}

/// This structure provides a possibility to calculate an SHA-256 hash digest
/// for a stream of data. Unlike the
/// [`Hash` structure](https://docs.rs/exonum/0.7.0/exonum/crypto/struct.Hash.html),
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
/// use exonum::crypto::HashStream;
///
/// let data: Vec<[u8; 5]> = vec![[1, 2, 3, 4, 5], [6, 7, 8, 9, 10]];
/// let mut hash_stream = HashStream::new();
/// for chunk in data {
///     hash_stream = hash_stream.update(&chunk);
/// }
/// let _ = hash_stream.hash();
/// ```
#[derive(Debug, Default)]
pub struct HashStream(HashState);

impl HashStream {
    /// Creates a new instance of `HashStream`.
    pub fn new() -> Self {
        HashStream(HashState::init())
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

/// This structure provides a possibility to create and/or verify Ed25519
/// digital signatures for a stream of data. If the data are split into several
/// chunks, the indicated chunks are added to the system and when adding is
/// complete, the data is signed.
///
/// Ed25519 is a signature system that ensures fast signing and key generation,
/// as well as security and collision resilience.
///
/// # Examples
///
/// The example below adds several data chunks to the system, generates a pair
/// of random public and secret keys, signs the data and verifies the signature.
///
/// ```rust
/// use exonum::crypto::{SignStream, gen_keypair};
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
pub struct SignStream(SignState);

impl SignStream {
    /// Creates a new instance of `SignStream`.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::crypto::SignStream;
    ///
    /// let stream = SignStream::new();
    /// ```
    pub fn new() -> Self {
        SignStream(SignState::init())
    }

    /// Adds a new `chunk` to the message that will eventually be signed and/or verified.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::crypto::SignStream;
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
    /// use exonum::crypto::{SignStream, gen_keypair};
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
    /// use exonum::crypto::{SignStream, gen_keypair};
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

implement_public_sodium_wrapper! {
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
/// use exonum::crypto;
///
/// # crypto::init();
/// let (public_key, _) = crypto::gen_keypair();
/// ```
    struct PublicKey, PublicKeySodium, PUBLIC_KEY_LENGTH
}

implement_private_sodium_wrapper! {
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
/// use exonum::crypto;
///
/// # crypto::init();
/// let (_, secret_key) = crypto::gen_keypair();
/// ```
    struct SecretKey, SecretKeySodium, SECRET_KEY_LENGTH
}

implement_public_sodium_wrapper! {
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
/// use exonum::crypto::{self, Hash};
///
/// let data = [1, 2, 3];
/// let hash_from_data = crypto::hash(&data);
/// let default_hash = Hash::default();
/// ```
    struct Hash, DigestSodium, HASH_SIZE
}

implement_public_sodium_wrapper! {
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
/// use exonum::crypto;
///
/// # crypto::init();
/// let (public_key, secret_key) = crypto::gen_keypair();
/// let data = [1, 2, 3];
/// let signature = crypto::sign(&data, &secret_key);
/// assert!(crypto::verify(&signature, &data, &public_key));
/// ```
    struct Signature, SignatureSodium, SIGNATURE_LENGTH
}

implement_private_sodium_wrapper! {
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
/// use exonum::crypto::{self, Seed};
///
/// # crypto::init();
/// let (public_key, secret_key) = crypto::gen_keypair_from_seed(&Seed::new([1; 32]));
/// ```
    struct Seed, SeedSodium, SEED_LENGTH
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

/// Returns a hash consisting of zeros.
impl Default for Hash {
    fn default() -> Self {
        Self::zero()
    }
}

impl CryptoHash for bool {
    fn hash(&self) -> Hash {
        hash(&[*self as u8])
    }
}

impl CryptoHash for u8 {
    fn hash(&self) -> Hash {
        hash(&[*self])
    }
}

impl CryptoHash for u16 {
    fn hash(&self) -> Hash {
        let mut v = [0; 2];
        LittleEndian::write_u16(&mut v, *self);
        hash(&v)
    }
}

impl CryptoHash for u32 {
    fn hash(&self) -> Hash {
        let mut v = [0; 4];
        LittleEndian::write_u32(&mut v, *self);
        hash(&v)
    }
}

impl CryptoHash for u64 {
    fn hash(&self) -> Hash {
        let mut v = [0; 8];
        LittleEndian::write_u64(&mut v, *self);
        hash(&v)
    }
}

impl CryptoHash for i8 {
    fn hash(&self) -> Hash {
        hash(&[*self as u8])
    }
}

impl CryptoHash for i16 {
    fn hash(&self) -> Hash {
        let mut v = [0; 2];
        LittleEndian::write_i16(&mut v, *self);
        hash(&v)
    }
}

impl CryptoHash for i32 {
    fn hash(&self) -> Hash {
        let mut v = [0; 4];
        LittleEndian::write_i32(&mut v, *self);
        hash(&v)
    }
}

impl CryptoHash for i64 {
    fn hash(&self) -> Hash {
        let mut v = [0; 8];
        LittleEndian::write_i64(&mut v, *self);
        hash(&v)
    }
}

const EMPTY_SLICE_HASH: Hash = Hash(DigestSodium([
    227, 176, 196, 66, 152, 252, 28, 20, 154, 251, 244, 200, 153, 111, 185, 36, 39, 174, 65, 228,
    100, 155, 147, 76, 164, 149, 153, 27, 120, 82, 184, 85,
]));

impl CryptoHash for () {
    fn hash(&self) -> Hash {
        EMPTY_SLICE_HASH
    }
}

impl CryptoHash for PublicKey {
    fn hash(&self) -> Hash {
        hash(self.as_ref())
    }
}

impl CryptoHash for Vec<u8> {
    fn hash(&self) -> Hash {
        hash(self)
    }
}

impl CryptoHash for String {
    fn hash(&self) -> Hash {
        hash(self.as_ref())
    }
}

impl CryptoHash for SystemTime {
    fn hash(&self) -> Hash {
        let duration = self.duration_since(UNIX_EPOCH)
            .expect("time value is later than 1970-01-01 00:00:00 UTC.");
        let secs = duration.as_secs();
        let nanos = duration.subsec_nanos();

        let mut buffer = [0_u8; 12];
        LittleEndian::write_u64(&mut buffer[0..8], secs);
        LittleEndian::write_u32(&mut buffer[8..12], nanos);
        hash(&buffer)
    }
}

impl CryptoHash for DateTime<Utc> {
    fn hash(&self) -> Hash {
        let secs = self.timestamp();
        let nanos = self.timestamp_subsec_nanos();

        let mut buffer = vec![0; 12];
        LittleEndian::write_i64(&mut buffer[0..8], secs);
        LittleEndian::write_u32(&mut buffer[8..12], nanos);
        buffer.hash()
    }
}

impl CryptoHash for Duration {
    fn hash(&self) -> Hash {
        let mut buffer = vec![0; Self::field_size() as usize];
        let from: Offset = 0;
        let to: Offset = Self::field_size();
        self.write(&mut buffer, from, to);
        buffer.hash()
    }
}

impl CryptoHash for Round {
    fn hash(&self) -> Hash {
        self.0.hash()
    }
}

impl CryptoHash for Uuid {
    fn hash(&self) -> Hash {
        hash(self.as_bytes())
    }
}

impl CryptoHash for Decimal {
    fn hash(&self) -> Hash {
        hash(&self.serialize())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use serde::de::DeserializeOwned;
    use serde_json;

    use encoding::serialize::FromHex;

    #[test]
    fn to_from_hex_hash() {
        let original = hash(&[]);
        let from_hex = Hash::from_hex(original.to_hex()).unwrap();
        assert_eq!(original, from_hex);
    }

    #[test]
    fn zero_hash() {
        let hash = Hash::zero();
        assert_eq!(hash.as_ref(), [0; 32]);
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
        assert_serialize_deserialize(&Hash::new([207; 32]));
    }

    #[test]
    fn serialize_deserialize_public_key() {
        assert_serialize_deserialize(&PublicKey::new([208; 32]));
    }

    #[test]
    fn serialize_deserialize_signature() {
        assert_serialize_deserialize(&Signature::new([209; 64]));
    }

    #[test]
    fn serialize_deserialize_seed() {
        assert_serialize_deserialize(&Seed::new([210; 32]));
    }

    #[test]
    fn serialize_deserialize_secret_key() {
        assert_serialize_deserialize(&SecretKey::new([211; 64]));
    }

    #[test]
    fn debug_format() {
        // Check zero padding
        let hash = Hash::new([1; 32]);
        assert_eq!(format!("{:?}", &hash), "Hash(01010101)");

        let pk = PublicKey::new([15; 32]);
        assert_eq!(format!("{:?}", &pk), "PublicKey(0F0F0F0F)");
        let sk = SecretKey::new([8; 64]);
        assert_eq!(format!("{:?}", &sk), "SecretKey(08080808...)");
        let signature = Signature::new([10; 64]);
        assert_eq!(format!("{:?}", &signature), "Signature(0A0A0A0A)");
        let seed = Seed::new([4; 32]);
        assert_eq!(format!("{:?}", &seed), "Seed(04040404...)");

        // Check no padding
        let hash = Hash::new([128; 32]);
        assert_eq!(format!("{:?}", &hash), "Hash(80808080)");
        let sk = SecretKey::new([255; 64]);
        assert_eq!(format!("{:?}", &sk), "SecretKey(FFFFFFFF...)");
    }

    #[test]
    fn range_sodium() {
        let h = hash(&[]);
        let sub_range = &h[10..20];
        assert_eq!(
            &[244u8, 200, 153, 111, 185, 36, 39, 174, 65, 228],
            sub_range
        );
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

    #[test]
    fn empty_slice_hash() {
        assert_eq!(EMPTY_SLICE_HASH, hash(&[]));
    }

    fn assert_serialize_deserialize<T>(original_value: &T)
    where
        T: Serialize + DeserializeOwned + PartialEq + fmt::Debug,
    {
        let json = serde_json::to_string(original_value).unwrap();
        let deserialized_value: T = serde_json::from_str(&json).unwrap();
        assert_eq!(*original_value, deserialized_value);
    }
}
