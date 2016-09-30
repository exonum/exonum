#![feature(type_ascription)]
#![feature(custom_derive)]
#![feature(plugin)]
#![plugin(serde_macros)]
#![feature(question_mark)]

extern crate time;
extern crate serde;
extern crate toml;
extern crate base64;
extern crate exonum;

pub mod config;
pub mod config_file;
pub mod blockchain_explorer;

use exonum::crypto::{Hash, PublicKey, SecretKey};

pub trait Base64Value: Sized {
    fn to_base64(&self) -> String;
    fn from_base64<T: AsRef<str>>(v: T) -> Result<Self, base64::Base64Error>;
}

impl Base64Value for Hash {
    fn to_base64(&self) -> String {
        base64::encode(self.as_ref())
    }
    fn from_base64<T: AsRef<str>>(v: T) -> Result<Self, base64::Base64Error> {
        let bytes = base64::decode(v.as_ref())?;
        if let Some(hash) = Hash::from_slice(bytes.as_ref()) {
            Ok(hash)
        } else {
            Err(base64::Base64Error::InvalidLength)
        }
    }
}

impl Base64Value for PublicKey {
    fn to_base64(&self) -> String {
        base64::encode(self.as_ref())
    }
    fn from_base64<T: AsRef<str>>(v: T) -> Result<Self, base64::Base64Error> {
        let bytes = base64::decode(v.as_ref())?;
        if let Some(hash) = Self::from_slice(bytes.as_ref()) {
            Ok(hash)
        } else {
            Err(base64::Base64Error::InvalidLength)
        }
    }
}

impl Base64Value for SecretKey {
    fn to_base64(&self) -> String {
        base64::encode(self.0.as_ref())
    }
    fn from_base64<T: AsRef<str>>(v: T) -> Result<Self, base64::Base64Error> {
        let bytes = base64::decode(v.as_ref())?;
        if let Some(hash) = Self::from_slice(bytes.as_ref()) {
            Ok(hash)
        } else {
            Err(base64::Base64Error::InvalidLength)
        }
    }
}

#[cfg(test)]
mod tests {
    use exonum::crypto::{hash, gen_keypair, Hash, PublicKey, SecretKey};
    use super::Base64Value;

    #[test]
    fn test_hash() {
        let h = hash(&[]);
        let h1 = Hash::from_base64(h.to_base64()).unwrap();
        assert_eq!(h1, h);
    }

    #[test]
    fn test_keys() {
        let (p, s) = gen_keypair();
        let p1 = PublicKey::from_base64(p.to_base64()).unwrap();
        let s1 = SecretKey::from_base64(s.to_base64()).unwrap();
        assert_eq!(p1, p);
        assert_eq!(s1, s);
    }
}