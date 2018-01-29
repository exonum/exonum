// Copyright 2017 The Exonum Team
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

//! Cryptography related types and functions.
//!
//! [Sodium library](https://github.com/jedisct1/libsodium) is used under the hood through
//! [sodiumoxide rust bindings](https://github.com/dnaq/sodiumoxide).

use std::default::Default;
use std::ops::{Index, Range, RangeFrom, RangeFull, RangeTo};
use std::fmt;

use sodiumoxide::crypto::sign::ed25519::{gen_keypair as gen_keypair_sodium, keypair_from_seed,
                                         sign_detached, verify_detached,
                                         PublicKey as PublicKeySodium,
                                         SecretKey as SecretKeySodium, Seed as SeedSodium,
                                         Signature as SignatureSodium, State as SignState};
use sodiumoxide::crypto::hash::sha256::{hash as hash_sodium, Digest, State as HashState};
use sodiumoxide;
use serde::{Serialize, Serializer};
use serde::de::{self, Deserialize, Deserializer, Visitor};

use encoding::serialize::FromHex;

pub use sodiumoxide::crypto::sign::ed25519::{PUBLICKEYBYTES as PUBLIC_KEY_LENGTH,
                                             SECRETKEYBYTES as SECRET_KEY_LENGTH,
                                             SEEDBYTES as SEED_LENGTH,
                                             SIGNATUREBYTES as SIGNATURE_LENGTH};
pub use sodiumoxide::crypto::hash::sha256::DIGESTBYTES as HASH_SIZE;

/// The size to crop the string in debug messages.
const BYTES_IN_DEBUG: usize = 4;

/// Signs slice of bytes using the signer's secret key. Returns the resulting `Signature`.
///
/// # Examples
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
/// ```
/// use exonum::crypto::{self, Seed};
///
/// # crypto::init();
/// let (public_key, secret_key) = crypto::gen_keypair_from_seed(&Seed::new([1; 32]));
/// # drop(public_key);
/// # drop(secret_key);
/// ```
pub fn gen_keypair_from_seed(seed: &Seed) -> (PublicKey, SecretKey) {
    let (sod_pub_key, sod_secr_key) = keypair_from_seed(&seed.0);
    (PublicKey(sod_pub_key), SecretKey(sod_secr_key))
}

/// Generates a secret key and a corresponding public key using a cryptographically secure
/// pseudo-random number generator.
///
/// # Examples
///
/// ```
/// use exonum::crypto;
///
/// # crypto::init();
/// let (public_key, secret_key) = crypto::gen_keypair();
/// # drop(public_key);
/// # drop(secret_key);
/// ```
pub fn gen_keypair() -> (PublicKey, SecretKey) {
    let (pubkey, secrkey) = gen_keypair_sodium();
    (PublicKey(pubkey), SecretKey(secrkey))
}

/// Verifies that `data` is signed with a secret key corresponding to the given public key.
///
/// # Examples
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

/// Calculates an SHA-256 hash digest of a bytes slice.
///
/// # Examples
///
/// ```
/// use exonum::crypto;
///
/// # crypto::init();
/// let data = [1, 2, 3];
/// let hash = crypto::hash(&data);
/// # drop(hash);
/// ```
pub fn hash(data: &[u8]) -> Hash {
    let dig = hash_sodium(data);
    Hash(dig)
}

/// Initializes the sodium library and chooses faster versions of the primitives if possible.
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

/// This structure provides a possibility to calculate a SHA-256 hash digest
/// for a stream of data.
///
/// # Example
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

    /// Returns the hash of data supplied to the stream so far.
    pub fn hash(self) -> Hash {
        let dig = self.0.finalize();
        Hash(dig)
    }
}

/// This structure provides a possibility to create and/or verify Ed25519 digital signatures
/// for a stream of data.
///
/// # Example
///
/// ```rust
/// use exonum::crypto::{SignStream, gen_keypair};
///
/// let data: Vec<[u8; 5]> = vec![[1, 2, 3, 4, 5], [6, 7, 8, 9, 10]];
/// let (pk, sk) = gen_keypair();
/// let mut create_stream = SignStream::new();
/// let mut verify_stream = SignStream::new();
/// for chunk in data {
///     create_stream = create_stream.update(&chunk);
///     verify_stream = verify_stream.update(&chunk);
/// }
/// let file_sign = create_stream.sign(&sk);
/// assert!(verify_stream.verify(&file_sign, &pk));
/// ```
#[derive(Debug, Default)]
pub struct SignStream(SignState);

impl SignStream {
    /// Creates a new instance of `SignStream`.
    pub fn new() -> Self {
        SignStream(SignState::init())
    }

    /// Adds a new `chunk` to the message that will eventually be signed and/or verified.
    pub fn update(mut self, chunk: &[u8]) -> Self {
        self.0.update(chunk);
        self
    }

    /// Computes and returns a signature for the previously supplied message
    /// using the given `secret_key`.
    pub fn sign(&mut self, secret_key: &SecretKey) -> Signature {
        Signature(self.0.finalize(&secret_key.0))
    }

    /// Verifies that `sig` is a valid signature for the previously supplied message
    /// using the given `public_key`.
    pub fn verify(&mut self, sig: &Signature, public_key: &PublicKey) -> bool {
        self.0.verify(&sig.0, &public_key.0)
    }
}

macro_rules! implement_public_sodium_wrapper {
    ($(#[$attr:meta])* struct $name:ident, $name_from:ident, $size:expr) => (
    #[derive(PartialEq, Eq, Clone, Copy, PartialOrd, Ord, Hash)]
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
        pub fn new(ba: [u8; $size]) -> Self {
            $name($name_from(ba))
        }

        /// Creates a new instance from bytes slice.
        pub fn from_slice(bs: &[u8]) -> Option<Self> {
            $name_from::from_slice(bs).map($name)
        }

        /// Returns the hex representation of the binary data.
        /// Lower case letters are used (e.g. f9b4ca).
        pub fn to_hex(&self) -> String {
            $crate::encoding::serialize::encode_hex(self)
        }
    }

    impl AsRef<[u8]> for $name {
        fn as_ref(&self) -> &[u8] {
            self.0.as_ref()
        }
    }

    impl ::std::str::FromStr for $name {
        type Err = ::encoding::serialize::FromHexError;
        fn from_str(s: &str) -> Result<Self, Self::Err> {
            $name::from_hex(s)
        }
    }

    impl ToString for $name {
        fn to_string(&self) -> String {
            self.to_hex()
        }
    }

    impl fmt::Debug for $name {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, stringify!($name))?;
            write!(f, "(")?;
            for i in &self[0..BYTES_IN_DEBUG] {
                write!(f, "{:02X}", i)?
            }
            write!(f, ")")
        }
    }
    )
}

macro_rules! implement_private_sodium_wrapper {
    ($(#[$attr:meta])* struct $name:ident, $name_from:ident, $size:expr) => (
    #[derive(Clone, PartialEq, Eq)]
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
        pub fn new(ba: [u8; $size]) -> Self {
            $name($name_from(ba))
        }

        /// Creates a new instance from bytes slice.
        pub fn from_slice(bs: &[u8]) -> Option<Self> {
            $name_from::from_slice(bs).map($name)
        }

        /// Returns the hex representation of the binary data.
        /// Lower case letters are used (e.g. f9b4ca).
        pub fn to_hex(&self) -> String {
            $crate::encoding::serialize::encode_hex(&self[..])
        }
    }

    impl fmt::Debug for $name {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, stringify!($name))?;
            write!(f, "(")?;
            for i in &self[0..BYTES_IN_DEBUG] {
                write!(f, "{:02X}", i)?
            }
            write!(f, "...)")
        }
    }

    impl $crate::encoding::serialize::ToHex for $name {
        fn write_hex<W: ::std::fmt::Write>(&self, w: &mut W) -> ::std::fmt::Result {
            (self.0).0.as_ref().write_hex(w)
        }

        fn write_hex_upper<W: ::std::fmt::Write>(&self, w: &mut W) -> ::std::fmt::Result {
            (self.0).0.as_ref().write_hex_upper(w)
        }
    }
    )
}

implement_public_sodium_wrapper! {
/// Ed25519 public key used to verify digital signatures.
///
/// # Examples
///
/// ```
/// use exonum::crypto;
///
/// # crypto::init();
/// let (public_key, _) = crypto::gen_keypair();
/// # drop(public_key);
/// ```
    struct PublicKey, PublicKeySodium, PUBLIC_KEY_LENGTH
}

implement_private_sodium_wrapper! {
/// Ed25519 secret key used to create digital signatures over messages.
///
/// # Examples
///
/// ```
/// use exonum::crypto;
///
/// # crypto::init();
/// let (_, secret_key) = crypto::gen_keypair();
/// # drop(secret_key);
/// ```
    struct SecretKey, SecretKeySodium, SECRET_KEY_LENGTH
}

implement_public_sodium_wrapper! {
/// SHA-256 hash.
///
/// # Examples
///
/// ```
/// use exonum::crypto::{self, Hash};
///
/// let data = [1, 2, 3];
/// let hash_from_data = crypto::hash(&data);
/// let default_hash = Hash::default();
/// # drop(hash_from_data);
/// # drop(default_hash);
/// ```
    struct Hash, Digest, HASH_SIZE
}

implement_public_sodium_wrapper! {
/// Ed25519 digital signature.
///
/// # Examples
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
/// Ed25519 seed that can be used for deterministic keypair generation.
///
/// # Examples
///
/// ```
/// use exonum::crypto::{self, Seed};
///
/// # crypto::init();
/// let (public_key, secret_key) = crypto::gen_keypair_from_seed(&Seed::new([1; 32]));
/// # drop(public_key);
/// # drop(secret_key);
/// ```
    struct Seed, SeedSodium, SEED_LENGTH
}

macro_rules! implement_serde {
($name:ident) => (
    impl $crate::encoding::serialize::FromHex for $name {
        type Error = $crate::encoding::serialize::FromHexError;

        fn from_hex<T: AsRef<[u8]>>(v: T) -> Result<Self, Self::Error> {
            let bytes = Vec::<u8>::from_hex(v)?;
            if let Some(self_value) = Self::from_slice(bytes.as_ref()) {
                Ok(self_value)
            } else {
                Err($crate::encoding::serialize::FromHexError::InvalidStringLength)
            }
        }
    }

    impl Serialize for $name
    {
        fn serialize<S>(&self, ser:S) -> Result<S::Ok, S::Error>
        where S: Serializer
        {
            let hex_string = $crate::encoding::serialize::encode_hex(&self[..]);
            ser.serialize_str(&hex_string)
        }
    }

    impl<'de> Deserialize<'de> for $name
    {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: Deserializer<'de>
        {
            struct HexVisitor;

            impl<'v> Visitor<'v> for HexVisitor
            {
                type Value = $name;
                fn expecting (&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
                    write!(fmt, "expecting str.")
                }
                fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
                where E: de::Error
                {
                    $name::from_hex(s).map_err(|_| de::Error::custom("Invalid hex"))
                }
            }
            deserializer.deserialize_str(HexVisitor)
        }
    }
    )
}

implement_serde! {Hash}
implement_serde! {PublicKey}
implement_serde! {SecretKey}
implement_serde! {Seed}
implement_serde! {Signature}

macro_rules! implement_index_traits {
    ($newtype:ident) => (
        impl Index<Range<usize>> for $newtype {
            type Output = [u8];
            fn index(&self, _index: Range<usize>) -> &[u8] {
                let inner  = &self.0;
                inner.0.index(_index)
            }
        }
        impl Index<RangeTo<usize>> for $newtype {
            type Output = [u8];
            fn index(&self, _index: RangeTo<usize>) -> &[u8] {
                let inner  = &self.0;
                inner.0.index(_index)
            }
        }
        impl Index<RangeFrom<usize>> for $newtype {
            type Output = [u8];
            fn index(&self, _index: RangeFrom<usize>) -> &[u8] {
                let inner  = &self.0;
                inner.0.index(_index)
            }
        }
        impl Index<RangeFull> for $newtype {
            type Output = [u8];
            fn index(&self, _index: RangeFull) -> &[u8] {
                let inner  = &self.0;
                inner.0.index(_index)
            }
        })
}
implement_index_traits! {Hash}
implement_index_traits! {PublicKey}
implement_index_traits! {SecretKey}
implement_index_traits! {Seed}
implement_index_traits! {Signature}

/// Returns hash consisting of zeros.
impl Default for Hash {
    fn default() -> Hash {
        Hash::zero()
    }
}

#[cfg(test)]
mod tests {
    use serde_json;
    use encoding::serialize::FromHex;
    use super::{gen_keypair, hash, Hash, HashStream, PublicKey, SecretKey, Seed, SignStream,
                Signature};

    #[test]
    fn test_hash() {
        let h = hash(&[]);
        let h1 = Hash::from_hex(h.to_hex()).unwrap();
        assert_eq!(h1, h);
        let h = Hash::zero();
        assert_eq!(*h.as_ref(), [0; 32]);
    }

    #[test]
    fn test_keys() {
        let (p, s) = gen_keypair();
        let p1 = PublicKey::from_hex(p.to_hex()).unwrap();
        let s1 = SecretKey::from_hex(s.to_hex()).unwrap();
        assert_eq!(p1, p);
        assert_eq!(s1, s);
    }

    #[test]
    fn test_ser_deser() {
        let h = Hash::new([207; 32]);
        let json_h = serde_json::to_string(&h).unwrap();
        let h1 = serde_json::from_str(&json_h).unwrap();
        assert_eq!(h, h1);

        let h = PublicKey::new([208; 32]);
        let json_h = serde_json::to_string(&h).unwrap();
        let h1 = serde_json::from_str(&json_h).unwrap();
        assert_eq!(h, h1);

        let h = Signature::new([209; 64]);
        let json_h = serde_json::to_string(&h).unwrap();
        let h1 = serde_json::from_str(&json_h).unwrap();
        assert_eq!(h, h1);

        let h = Seed::new([210; 32]);
        let json_h = serde_json::to_string(&h).unwrap();
        let h1 = serde_json::from_str(&json_h).unwrap();
        assert_eq!(h, h1);

        let h = SecretKey::new([211; 64]);
        let json_h = serde_json::to_string(&h).unwrap();
        let h1 = serde_json::from_str(&json_h).unwrap();
        assert_eq!(h, h1);
    }

    #[test]
    fn test_debug_format() {
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
    fn test_range_sodium() {
        let h = hash(&[]);
        let sub_range = &h[10..20];
        assert_eq!(
            &[244u8, 200, 153, 111, 185, 36, 39, 174, 65, 228],
            sub_range
        );
    }

    #[test]
    fn test_hash_streaming_zero() {
        let h1 = hash(&[]);
        let state = HashStream::new();
        let h2 = state.update(&[]).hash();
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_hash_streaming_chunks() {
        let data: [u8; 10] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 0];
        let h1 = hash(&data);
        let state = HashStream::new();
        let h2 = state.update(&data[..5]).update(&data[5..]).hash();
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_sign_streaming_zero() {
        let (pk, sk) = gen_keypair();
        let mut creation_stream = SignStream::new().update(&[]);
        let sig = creation_stream.sign(&sk);
        let mut verified_stream = SignStream::new().update(&[]);
        assert!(verified_stream.verify(&sig, &pk));
    }

    #[test]
    fn test_sign_streaming_chunks() {
        let data: [u8; 10] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 0];
        let (pk, sk) = gen_keypair();
        let mut creation_stream = SignStream::new().update(&data[..5]).update(&data[5..]);
        let sig = creation_stream.sign(&sk);
        let mut verified_stream = SignStream::new().update(&data[..5]).update(&data[5..]);
        assert!(verified_stream.verify(&sig, &pk));
    }
}
