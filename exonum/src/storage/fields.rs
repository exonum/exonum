use std::mem;
use std::sync::Arc;
use byteorder::{ByteOrder, BigEndian};
use super::Error;

use ::crypto::{Hash, hash};
use ::messages::{RawMessage, MessageBuffer, Message, FromRaw};
use serde::Deserialize;
use serde_json::Value;
use serde_json::value::from_value;

pub trait DeserializeFromJson: Sized {
    fn deserialize(json: &Value) -> Result<Self, Error>;
}

impl<T: Deserialize> DeserializeFromJson for T {
    fn deserialize(json: &Value) -> Result<Self, Error> {
        from_value(json.clone())
            .map_err(|e| Error::new(format!("Error deserializing from json: {}", e)))
    }
}

#[derive(Clone)]
pub struct HeightBytes(pub [u8; 32]);

pub trait StorageValue {
    fn serialize(self) -> Vec<u8>;
    fn deserialize(v: Vec<u8>) -> Self;
    fn hash(&self) -> Hash;
}

impl StorageValue for u16 {
    fn serialize(self) -> Vec<u8> {
        let mut v = vec![0; mem::size_of::<u16>()];
        BigEndian::write_u16(&mut v, self);
        v
    }

    fn deserialize(v: Vec<u8>) -> Self {
        BigEndian::read_u16(&v)
    }

    fn hash(&self) -> Hash {
        let mut v = vec![0; mem::size_of::<u16>()];
        BigEndian::write_u16(&mut v, *self);
        hash(&v)
    }
}

impl StorageValue for u32 {
    // TODO: return Cow<[u8]>
    fn serialize(self) -> Vec<u8> {
        let mut v = vec![0; mem::size_of::<u32>()];
        BigEndian::write_u32(&mut v, self);
        v
    }

    fn deserialize(v: Vec<u8>) -> Self {
        BigEndian::read_u32(&v)
    }

    fn hash(&self) -> Hash {
        let mut v = vec![0; mem::size_of::<u32>()];
        BigEndian::write_u32(&mut v, *self);
        hash(&v)
    }
}

impl StorageValue for u64 {
    fn serialize(self) -> Vec<u8> {
        let mut v = vec![0; mem::size_of::<u64>()];
        BigEndian::write_u64(&mut v, self);
        v
    }

    fn deserialize(v: Vec<u8>) -> Self {
        BigEndian::read_u64(&v)
    }

    fn hash(&self) -> Hash {
        let mut v = vec![0; mem::size_of::<u64>()];
        BigEndian::write_u64(&mut v, *self);
        hash(&v)
    }
}

impl StorageValue for i64 {
    fn serialize(self) -> Vec<u8> {
        let mut v = vec![0; mem::size_of::<i64>()];
        BigEndian::write_i64(&mut v, self);
        v
    }

    fn deserialize(v: Vec<u8>) -> Self {
        BigEndian::read_i64(&v)
    }

    fn hash(&self) -> Hash {
        let mut v = vec![0; mem::size_of::<i64>()];
        BigEndian::write_i64(&mut v, *self);
        hash(&v)
    }
}

impl StorageValue for Hash {
    fn serialize(self) -> Vec<u8> {
        self.as_ref().to_vec()
    }

    fn deserialize(v: Vec<u8>) -> Self {
        Hash::from_slice(&v).unwrap()
    }

    fn hash(&self) -> Hash {
        hash(self.as_ref())
    }
}

impl StorageValue for RawMessage {
    fn serialize(self) -> Vec<u8> {
        self.as_ref().as_ref().to_vec()
    }

    fn deserialize(v: Vec<u8>) -> Self {
        Arc::new(MessageBuffer::from_vec(v))
    }

    fn hash(&self) -> Hash {
        self.as_ref().hash()
    }
}

impl<T> StorageValue for T
    where T: FromRaw
{
    fn serialize(self) -> Vec<u8> {
        self.raw().as_ref().as_ref().to_vec()
    }

    fn deserialize(v: Vec<u8>) -> Self {
        FromRaw::from_raw(Arc::new(MessageBuffer::from_vec(v))).unwrap()
    }

    fn hash(&self) -> Hash {
        <Self as Message>::hash(self)
    }
}

impl StorageValue for Vec<u8> {
    fn serialize(self) -> Vec<u8> {
        self
    }

    fn deserialize(v: Vec<u8>) -> Self {
        v
    }

    fn hash(&self) -> Hash {
        hash(self)
    }
}

impl AsRef<[u8]> for HeightBytes {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl From<u64> for HeightBytes {
    fn from(b: u64) -> HeightBytes {
        let mut v = [0u8; 32];
        BigEndian::write_u64(&mut v, b);
        HeightBytes(v)
    }
}

impl From<HeightBytes> for u64 {
    fn from(b: HeightBytes) -> u64 {
        BigEndian::read_u64(b.as_ref())
    }
}

impl StorageValue for HeightBytes {
    fn serialize(self) -> Vec<u8> {
        self.as_ref().to_vec()
    }

    fn deserialize(v: Vec<u8>) -> Self {
        let mut b = [0u8; 32];
        b.clone_from_slice(v.as_slice());
        HeightBytes(b)
    }

    fn hash(&self) -> Hash {
        hash(self.as_ref())
    }
}
