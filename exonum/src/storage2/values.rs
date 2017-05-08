use byteorder::{ByteOrder, LittleEndian};

use std::mem;
use std::sync::Arc;

use crypto::{Hash, hash, PublicKey};
use messages::{RawMessage, MessageBuffer, Message};

pub trait StorageValue : Sized {
    fn hash(&self) -> Hash;
    fn into_vec(self) -> Vec<u8>;
    fn from_slice(value: &[u8]) -> Self;
    fn from_vec(value: Vec<u8>) -> Self {
        Self::from_slice(&value)
    }
}

impl StorageValue for () {
    fn into_vec(self) -> Vec<u8> {
        Vec::new()
    }

    fn from_slice(_value: &[u8]) -> Self {
        ()
    }

    fn hash(&self) -> Hash {
        Hash::zero()
    }
}

impl StorageValue for u8 {
    fn into_vec(self) -> Vec<u8> {
        vec![self]
    }

    fn from_slice(value: &[u8]) -> Self {
        value[0]
    }

    fn hash(&self) -> Hash {
        hash(vec![*self].as_slice())
    }
}

impl StorageValue for u16 {
    fn into_vec(self) -> Vec<u8> {
        let mut v = vec![0; mem::size_of::<u16>()];
        LittleEndian::write_u16(&mut v, self);
        v
    }

    fn from_slice(value: &[u8]) -> Self {
        LittleEndian::read_u16(value)
    }

    fn hash(&self) -> Hash {
        let mut v = vec![0; mem::size_of::<u16>()];
        LittleEndian::write_u16(&mut v, *self);
        hash(&v)
    }
}

impl StorageValue for u32 {
    fn into_vec(self) -> Vec<u8> {
        let mut v = vec![0; mem::size_of::<u32>()];
        LittleEndian::write_u32(&mut v, self);
        v
    }

    fn from_slice(value: &[u8]) -> Self {
        LittleEndian::read_u32(value)
    }

    fn hash(&self) -> Hash {
        let mut v = vec![0; mem::size_of::<u32>()];
        LittleEndian::write_u32(&mut v, *self);
        hash(&v)
    }
}

impl StorageValue for u64 {
    fn into_vec(self) -> Vec<u8> {
        let mut v = vec![0; mem::size_of::<u64>()];
        LittleEndian::write_u64(&mut v, self);
        v
    }

    fn from_slice(value: &[u8]) -> Self {
        LittleEndian::read_u64(value)
    }

    fn hash(&self) -> Hash {
        let mut v = vec![0; mem::size_of::<u64>()];
        LittleEndian::write_u64(&mut v, *self);
        hash(&v)
    }
}

impl StorageValue for i8 {
    fn into_vec(self) -> Vec<u8> {
        vec![self as u8]
    }

    fn from_slice(value: &[u8]) -> Self {
        value[0] as i8
    }

    fn hash(&self) -> Hash {
        hash(vec![*self as u8].as_slice())
    }
}

impl StorageValue for i16 {
    fn into_vec(self) -> Vec<u8> {
        let mut v = vec![0; mem::size_of::<i16>()];
        LittleEndian::write_i16(&mut v, self);
        v
    }

    fn from_slice(value: &[u8]) -> Self {
        LittleEndian::read_i16(value)
    }

    fn hash(&self) -> Hash {
        let mut v = vec![0; mem::size_of::<i16>()];
        LittleEndian::write_i16(&mut v, *self);
        hash(&v)
    }
}

impl StorageValue for i32 {
    fn into_vec(self) -> Vec<u8> {
        let mut v = vec![0; mem::size_of::<i32>()];
        LittleEndian::write_i32(&mut v, self);
        v
    }

    fn from_slice(value: &[u8]) -> Self {
        LittleEndian::read_i32(value)
    }

    fn hash(&self) -> Hash {
        let mut v = vec![0; mem::size_of::<i32>()];
        LittleEndian::write_i32(&mut v, *self);
        hash(&v)
    }
}

impl StorageValue for i64 {
    fn into_vec(self) -> Vec<u8> {
        let mut v = vec![0; mem::size_of::<i64>()];
        LittleEndian::write_i64(&mut v, self);
        v
    }

    fn from_slice(value: &[u8]) -> Self {
        LittleEndian::read_i64(value)
    }

    fn hash(&self) -> Hash {
        let mut v = vec![0; mem::size_of::<i64>()];
        LittleEndian::write_i64(&mut v, *self);
        hash(&v)
    }
}

impl StorageValue for Hash {
    fn into_vec(self) -> Vec<u8> {
        self.as_ref().to_vec()
    }

    fn from_slice(value: &[u8]) -> Self {
        Hash::from_slice(value).unwrap()
    }

    fn hash(&self) -> Hash {
        *self
    }
}

impl StorageValue for PublicKey {
    fn into_vec(self) -> Vec<u8> {
        self.as_ref().to_vec()
    }

    fn from_slice(value: &[u8]) -> Self {
        PublicKey::from_slice(value).unwrap()
    }

    fn hash(&self) -> Hash {
        hash(self.as_ref())
    }
}

impl StorageValue for RawMessage {
    fn into_vec(self) -> Vec<u8> {
        self.as_ref().as_ref().to_vec()
    }

    fn from_slice(value: &[u8]) -> Self {
        Self::from_vec(value.to_vec())
    }

    fn from_vec(value: Vec<u8>) -> Self {
        Arc::new(MessageBuffer::from_vec(value))
    }

    fn hash(&self) -> Hash {
        Message::hash(self)
    }
}

impl StorageValue for Vec<u8> {
    fn into_vec(self) -> Vec<u8> {
        self
    }

    fn from_slice(value: &[u8]) -> Self {
        value.to_vec()
    }

    fn from_vec(value: Vec<u8>) -> Self {
        value
    }

    fn hash(&self) -> Hash {
        hash(self)
    }
}

impl StorageValue for String {
    fn into_vec(self) -> Vec<u8> {
        String::into_bytes(self)
    }

    fn from_slice(value: &[u8]) -> Self {
        Self::from_vec(value.to_vec())
    }

    fn from_vec(value: Vec<u8>) -> Self {
        String::from_utf8(value).unwrap()
    }

    fn hash(&self) -> Hash {
        hash(self.as_ref())
    }
}
