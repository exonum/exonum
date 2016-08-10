use std::mem;
use std::sync::Arc;

use byteorder::{ByteOrder, BigEndian};

use ::crypto::{Hash, hash};
use ::messages::{MessageBuffer, Message, TxMessage};

pub trait StorageValue {
    fn serialize(self) -> Vec<u8>;
    fn deserialize(v: Vec<u8>) -> Self;
    fn hash(&self) -> Hash;
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
        let mut v = vec![0; mem::size_of::<u64>()];
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

impl StorageValue for Hash {
    fn serialize(self) -> Vec<u8> {
        self.as_ref().to_vec()
    }

    fn deserialize(v: Vec<u8>) -> Self {
        Hash::from_slice(&v).unwrap()
    }

    fn hash(&self) -> Hash {
        self.clone()
    }
}

impl<T> StorageValue for T
    where T: Message
{
    fn serialize(self) -> Vec<u8> {
        self.raw().as_ref().as_ref().to_vec()
    }

    fn deserialize(v: Vec<u8>) -> Self {
        Message::from_raw(Arc::new(MessageBuffer::from_vec(v))).unwrap()
    }
    
    fn hash(&self) -> Hash {
        self.hash()
    }
}

impl StorageValue for TxMessage {
    fn serialize(self) -> Vec<u8> {
        self.raw().as_ref().as_ref().to_vec()
    }

    fn deserialize(v: Vec<u8>) -> Self {
        TxMessage::from_raw(Arc::new(MessageBuffer::from_vec(v))).unwrap()
    }

    fn hash(&self) -> Hash {
        self.hash()
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
        hash(&self)
    }    
}
