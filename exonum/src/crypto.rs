pub use sodiumoxide::crypto::sign::ed25519::{PUBLICKEYBYTES as PUBLIC_KEY_LENGTH,
                                             SECRETKEYBYTES as SECRET_KEY_LENGTH,
                                             SIGNATUREBYTES as SIGNATURE_LENGTH,
                                             SEEDBYTES as SEED_LENGTH};
pub use sodiumoxide::crypto::hash::sha256::DIGESTBYTES as HASH_SIZE;
use sodiumoxide::crypto::sign::ed25519::{PublicKey as PublicKeySodium,
                                         SecretKey as SecretKeySodium, Seed as SeedSodium,
                                         Signature as SignatureSodium, sign_detached,
                                         verify_detached, gen_keypair as gen_keypair_sodium,
                                         keypair_from_seed};
use sodiumoxide::crypto::hash::sha256::{Digest, hash as hash_sodium};
use serde::{Serialize, Serializer, Deserialize, Deserializer};

pub use hex::{ToHex, FromHex, FromHexError};

pub fn sign(m: &[u8], secret_key: &SecretKey) -> Signature {
    let sodium_signature = sign_detached(m, &secret_key.inner);
    Signature { inner: sodium_signature }
}

pub fn gen_keypair_from_seed(seed: &Seed) -> (PublicKey, SecretKey) {
    let (sod_pub_key, sod_secr_key) = keypair_from_seed(&seed.inner);
    (PublicKey { inner: sod_pub_key }, SecretKey { inner: sod_secr_key })
}

pub fn gen_keypair() -> (PublicKey, SecretKey) {
    let (pubkey, secrkey) = gen_keypair_sodium();
    (PublicKey { inner: pubkey }, SecretKey { inner: secrkey })
}

pub fn verify(sig: &Signature, m: &[u8], pubkey: &PublicKey) -> bool {
    verify_detached(&sig.inner, m, &pubkey.inner)
}

pub fn hash(m: &[u8]) -> Hash {
    let dig = hash_sodium(m);
    Hash { inner: dig }
}

#[derive(PartialEq, Eq, Clone, Copy, PartialOrd, Ord, Hash, Debug)]
pub struct PublicKey {
    pub inner: PublicKeySodium,
}

impl PublicKey {
    pub fn from_slice(bs: &[u8]) -> Option<PublicKey> {
        PublicKeySodium::from_slice(bs).map(|pk| PublicKey { inner: pk })
    }
}
impl AsRef<[u8]> for PublicKey {
    fn as_ref(&self) -> &[u8] {
        self.inner.as_ref()
    }
}
impl Serialize for PublicKey {
    fn serialize<S>(&self, serializer: &mut S) -> Result<(), S::Error>
        where S: Serializer
    {
        self.inner.serialize(serializer)
    }
}

impl Deserialize for PublicKey {
    fn deserialize<D>(deserializer: &mut D) -> Result<Self, D::Error>
        where D: Deserializer
    {
        let pubkey = Deserialize::deserialize(deserializer)?;
        Ok(Self { inner: pubkey })
    }
}
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SecretKey {
    pub inner: SecretKeySodium,
}

impl Serialize for SecretKey {
    fn serialize<S>(&self, serializer: &mut S) -> Result<(), S::Error>
        where S: Serializer
    {
        self.inner.serialize(serializer)
    }
}

impl Deserialize for SecretKey {
    fn deserialize<D>(deserializer: &mut D) -> Result<Self, D::Error>
        where D: Deserializer
    {
        let secrkey: SecretKeySodium = Deserialize::deserialize(deserializer)?;
        Ok(Self { inner: secrkey })
    }
}
impl SecretKey {
    pub fn from_slice(bs: &[u8]) -> Option<SecretKey> {
        SecretKeySodium::from_slice(bs).map(|sk| SecretKey { inner: sk })
    }
}
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Seed {
    pub inner: SeedSodium,
}
impl Seed {
    pub fn from_slice(bs: &[u8]) -> Option<Seed> {
        SeedSodium::from_slice(bs).map(|seed| Seed { inner: seed })
    }
}

#[derive(PartialEq, Eq, Clone, Copy, PartialOrd, Ord, Hash, Debug)]
pub struct Signature {
    pub inner: SignatureSodium,
}
impl Signature {
    pub fn from_slice(bs: &[u8]) -> Option<Signature> {
        SignatureSodium::from_slice(bs).map(|sign| Signature { inner: sign })
    }
}
impl AsRef<[u8]> for Signature {
    fn as_ref(&self) -> &[u8] {
        self.inner.as_ref()
    }
}

#[derive(PartialEq, Eq, Clone, Copy, PartialOrd, Ord, Hash, Debug)]
pub struct Hash {
    pub inner: Digest,
}

impl Hash {
    pub fn from_slice(bs: &[u8]) -> Option<Hash> {
        Digest::from_slice(bs).map(|digest| Hash { inner: digest })
    }
}
impl AsRef<[u8]> for Hash {
    fn as_ref(&self) -> &[u8] {
        self.inner.as_ref()
    }
}

impl Serialize for Hash {
    fn serialize<S>(&self, serializer: &mut S) -> Result<(), S::Error>
        where S: Serializer
    {
        self.inner.serialize(serializer)
    }
}

impl Deserialize for Hash {
    fn deserialize<D>(deserializer: &mut D) -> Result<Self, D::Error>
        where D: Deserializer
    {
        let hash = Deserialize::deserialize(deserializer)?;
        Ok(Self { inner: hash })
    }
}

pub trait HexValue: Sized {
    fn to_hex(&self) -> String;
    fn from_hex<T: AsRef<str>>(v: T) -> Result<Self, FromHexError>;
}

impl HexValue for Vec<u8> {
    fn to_hex(&self) -> String {
        let r: &[u8] = self.as_ref();
        r.to_hex()
    }
    fn from_hex<T: AsRef<str>>(v: T) -> Result<Self, FromHexError> {
        FromHex::from_hex(v.as_ref())
    }
}

impl HexValue for Hash {
    fn to_hex(&self) -> String {
        self.as_ref().to_hex()
    }
    fn from_hex<T: AsRef<str>>(v: T) -> Result<Self, FromHexError> {
        let bytes: Vec<u8> = FromHex::from_hex(v.as_ref())?;
        if let Some(hash) = Hash::from_slice(bytes.as_ref()) {
            Ok(hash)
        } else {
            Err(FromHexError::InvalidHexLength)
        }
    }
}

impl HexValue for PublicKey {
    fn to_hex(&self) -> String {
        self.as_ref().to_hex()
    }
    fn from_hex<T: AsRef<str>>(v: T) -> Result<Self, FromHexError> {
        let bytes: Vec<u8> = FromHex::from_hex(v.as_ref())?;
        if let Some(hash) = Self::from_slice(bytes.as_ref()) {
            Ok(hash)
        } else {
            Err(FromHexError::InvalidHexLength)
        }
    }
}

impl HexValue for SecretKey {
    fn to_hex(&self) -> String {
        self.inner.0.as_ref().to_hex()
    }
    fn from_hex<T: AsRef<str>>(v: T) -> Result<Self, FromHexError> {
        let bytes: Vec<u8> = FromHex::from_hex(v.as_ref())?;
        if let Some(hash) = Self::from_slice(bytes.as_ref()) {
            Ok(hash)
        } else {
            Err(FromHexError::InvalidHexLength)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{hash, gen_keypair, Hash, PublicKey, SecretKey};
    use super::HexValue;

    #[test]
    fn test_hash() {
        let h = hash(&[]);
        let h1 = Hash::from_hex(h.to_hex()).unwrap();
        assert_eq!(h1, h);
    }

    #[test]
    fn test_keys() {
        let (p, s) = gen_keypair();
        let p1 = PublicKey::from_hex(p.to_hex()).unwrap();
        let s1 = SecretKey::from_hex(s.to_hex()).unwrap();
        assert_eq!(p1, p);
        assert_eq!(s1, s);
    }
}
