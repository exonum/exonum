use byteorder::{ByteOrder, LittleEndian};

use std::mem;
use std::sync::Arc;

use crypto::{Hash, hash};
use messages::{RawMessage, MessageBuffer, Message, FromRaw};

// TODO: add implementations for other primitives
// TODO: review signature

pub trait StorageValue {
    fn serialize(self) -> Vec<u8>;
    fn deserialize(value: Vec<u8>) -> Self;
    fn hash(&self) -> Hash;
}

impl StorageValue for () {
    fn serialize(self) -> Vec<u8> {
        Vec::new()
    }

    fn deserialize(value: Vec<u8>) -> Self {
        ()
    }

    fn hash(&self) -> Hash {
        Hash::zero()
    }
}

impl StorageValue for u16 {
    fn serialize(self) -> Vec<u8> {
        let mut v = vec![0; mem::size_of::<u16>()];
        LittleEndian::write_u16(&mut v, self);
        v
    }

    fn deserialize(value: Vec<u8>) -> Self {
        LittleEndian::read_u16(&value)
    }

    fn hash(&self) -> Hash {
        let mut v = vec![0; mem::size_of::<u16>()];
        LittleEndian::write_u16(&mut v, *self);
        hash(&v)
    }
}

impl StorageValue for u32 {
    fn serialize(self) -> Vec<u8> {
        let mut v = vec![0; mem::size_of::<u32>()];
        LittleEndian::write_u32(&mut v, self);
        v
    }

    fn deserialize(value: Vec<u8>) -> Self {
        LittleEndian::read_u32(&value)
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

    fn deserialize(value: Vec<u8>) -> Self {
        LittleEndian::read_u64(&value)
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

    fn deserialize(value: Vec<u8>) -> Self {
        LittleEndian::read_i64(&value)
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

    fn deserialize(value: Vec<u8>) -> Self {
        Hash::from_slice(&value).unwrap()
    }

    fn hash(&self) -> Hash {
        hash(self.as_ref())
    }
}

impl StorageValue for RawMessage {
    fn serialize(self) -> Vec<u8> {
        self.as_ref().as_ref().to_vec()
    }

    fn deserialize(value: Vec<u8>) -> Self {
        Arc::new(MessageBuffer::from_vec(value))
    }

    fn hash(&self) -> Hash {
        Message::hash(self)
    }
}

impl StorageValue for Vec<u8> {
    fn serialize(self) -> Vec<u8> {
        self
    }

    fn deserialize(value: Vec<u8>) -> Self {
        value
    }

    fn hash(&self) -> Hash {
        hash(self)
    }
}
