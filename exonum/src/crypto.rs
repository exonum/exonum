//! Cryptography related types and functions.
//!
//! [Sodium library](https://github.com/jedisct1/libsodium) is used under the hood through
//! [sodiumoxide rust bindings](https://github.com/dnaq/sodiumoxide).

use sodiumoxide::crypto::sign::ed25519::{PublicKey as PublicKeySodium,
                                         SecretKey as SecretKeySodium, Seed as SeedSodium,
                                         Signature as SignatureSodium, sign_detached,
                                         verify_detached, gen_keypair as gen_keypair_sodium,
                                         keypair_from_seed};
use sodiumoxide::crypto::hash::sha256::{Digest, hash as hash_sodium};
use sodiumoxide;
use serde::{Serialize, Serializer};
use serde::de::{self, Visitor, Deserialize, Deserializer};
use hex::{ToHex, FromHex};

use std::default::Default;
use std::ops::{Index, Range, RangeFrom, RangeTo, RangeFull};
use std::fmt;

pub use sodiumoxide::crypto::sign::ed25519::{PUBLICKEYBYTES as PUBLIC_KEY_LENGTH,
                                             SECRETKEYBYTES as SECRET_KEY_LENGTH,
                                             SIGNATUREBYTES as SIGNATURE_LENGTH,
                                             SEEDBYTES as SEED_LENGTH};
pub use sodiumoxide::crypto::hash::sha256::DIGESTBYTES as HASH_SIZE;

pub use encoding::serialize::{FromHexError, HexValue};

/// The size to crop the string in debug messages.
const BYTES_IN_DEBUG: usize = 4;

/// Signs slice of bytes using the signer's secret key. Returns the resulting `Signature`.
///
/// # Example
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
/// # Example
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

/// Randomly generates a secret key and a corresponding public key.
///
/// # Example
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
/// # Example
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

/// Calculates `SHA256` hash of bytes slice.
///
/// # Example
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
/// # Example
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
    }

    impl AsRef<[u8]> for $name {
        fn as_ref(&self) -> &[u8] {
            self.0.as_ref()
        }
    }

    impl fmt::Debug for $name {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, stringify!($name))?;
            write!(f, "(")?;
            for i in &self[0..BYTES_IN_DEBUG] {
                write!(f, "{:X}", i)?
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
    }

    impl fmt::Debug for $name {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, stringify!($name))?;
            write!(f, "(")?;
            for i in &self[0..BYTES_IN_DEBUG] {
                write!(f, "{:X}", i)?
            }
            write!(f, "...)")
        }
    }
    )
}

implement_public_sodium_wrapper! {
/// Public key used for verifying signatures.
///
/// # Example
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
/// Secret key used for signing.
////// # Example
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
/// SHA256 hash.
///
/// `Default` implementation for the `Hash` returns hash consisting of zeros.
///
/// # Example
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
/// Signature.
///
/// # Example
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
/// Seed that can be used for keypair generation.
///
/// # Example
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
    impl HexValue for $name {
        fn to_hex(&self) -> String {
            let inner = &self.0;
            inner.0.as_ref().to_hex()
        }

        fn from_hex<T: AsRef<str>>(v: T) -> Result<Self, FromHexError> {
            let bytes: Vec<u8> = FromHex::from_hex(v.as_ref())?;
            if let Some(self_value) = Self::from_slice(bytes.as_ref()) {
                Ok(self_value)
            } else {
                Err(FromHexError::InvalidHexLength)
            }
        }
    }

    impl Serialize for $name
    {
        fn serialize<S>(&self, ser:S) -> Result<S::Ok, S::Error>
        where S: Serializer
        {
            ser.serialize_str(&HexValue::to_hex(self))
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

impl HexValue for Vec<u8> {
    fn to_hex(&self) -> String {
        ToHex::to_hex(self)
    }
    fn from_hex<T: AsRef<str>>(v: T) -> Result<Self, FromHexError> {
        FromHex::from_hex(v.as_ref())
    }
}
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

impl Default for Hash {
    fn default() -> Hash {
        Hash::zero()
    }
}

#[cfg(test)]
mod tests {
    use super::{hash, gen_keypair, Hash, PublicKey, SecretKey, Seed, Signature};
    use super::HexValue;
    use serde_json;

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
    fn test_range_sodium() {
        let h = hash(&[]);
        let sub_range = &h[10..20];
        assert_eq!(&[244u8, 200, 153, 111, 185, 36, 39, 174, 65, 228],
                   sub_range);
    }
}
