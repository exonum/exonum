use std::mem;
use std::sync::Arc;
use byteorder::{ByteOrder, LittleEndian};
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

pub trait StorageValue {
    fn serialize(self) -> Vec<u8>;
    fn deserialize(v: Vec<u8>) -> Self;
    fn hash(&self) -> Hash;
}

impl StorageValue for u16 {
    fn serialize(self) -> Vec<u8> {
        let mut v = vec![0; mem::size_of::<u16>()];
        LittleEndian::write_u16(&mut v, self);
        v
    }

    fn deserialize(v: Vec<u8>) -> Self {
        LittleEndian::read_u16(&v)
    }

    fn hash(&self) -> Hash {
        let mut v = vec![0; mem::size_of::<u16>()];
        LittleEndian::write_u16(&mut v, *self);
        hash(&v)
    }
}

impl StorageValue for u32 {
    // TODO: return Cow<[u8]>
    fn serialize(self) -> Vec<u8> {
        let mut v = vec![0; mem::size_of::<u32>()];
        LittleEndian::write_u32(&mut v, self);
        v
    }

    fn deserialize(v: Vec<u8>) -> Self {
        LittleEndian::read_u32(&v)
    }

    fn hash(&self) -> Hash {
        let mut v = vec![0; mem::size_of::<u32>()];
        LittleEndian::write_u32(&mut v, *self);
        hash(&v)
    }
}

impl StorageValue for u64 {
    fn serialize(self) -> Vec<u8> {
        let mut v = vec![0; mem::size_of::<u64>()];
        LittleEndian::write_u64(&mut v, self);
        v
    }

    fn deserialize(v: Vec<u8>) -> Self {
        LittleEndian::read_u64(&v)
    }

    fn hash(&self) -> Hash {
        let mut v = vec![0; mem::size_of::<u64>()];
        LittleEndian::write_u64(&mut v, *self);
        hash(&v)
    }
}

impl StorageValue for i64 {
    fn serialize(self) -> Vec<u8> {
        let mut v = vec![0; mem::size_of::<i64>()];
        LittleEndian::write_i64(&mut v, self);
        v
    }

    fn deserialize(v: Vec<u8>) -> Self {
        LittleEndian::read_i64(&v)
    }

    fn hash(&self) -> Hash {
        let mut v = vec![0; mem::size_of::<i64>()];
        LittleEndian::write_i64(&mut v, *self);
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
        *self
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