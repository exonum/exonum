pub use sodiumoxide::crypto::sign::ed25519::{PublicKey, SecretKey, Seed, Signature,
                                             sign_detached as sign, verify_detached as verify,
                                             gen_keypair,
                                             keypair_from_seed as gen_keypair_from_seed,
                                             PUBLICKEYBYTES as PUBLIC_KEY_LENGTH,
                                             SECRETKEYBYTES as SECRET_KEY_LENGTH,
                                             SIGNATUREBYTES as SIGNATURE_LENGTH,
                                             SEEDBYTES as SEED_LENGTH};

pub use sodiumoxide::crypto::hash::sha256::{hash, Digest as Hash, DIGESTBYTES as HASH_SIZE};

use hex::{FromHex, FromHexError};
pub use hex::ToHex;

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
        self.0.as_ref().to_hex()
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
