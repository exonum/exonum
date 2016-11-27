use std::mem;
use std::sync::Arc;
use std::ops::Deref;
use std::marker::PhantomData;
use base64::{encode, decode};
use serde::{Serialize, Serializer};
use serde::de;
use serde::de::{Visitor, Deserialize, Deserializer};

use byteorder::{ByteOrder, BigEndian};

use ::crypto::{Hash, hash};
use ::messages::{MessageBuffer, Message, AnyTx};

#[derive(Clone)]
pub struct HeightBytes(pub [u8; 32]);

pub trait StorageValue {
    fn serialize(self) -> Vec<u8>;
    fn deserialize(v: Vec<u8>) -> Self;
    fn hash(&self) -> Hash;   
}

#[derive(Clone, Debug)]
pub struct Base64Field<T: StorageValue + Clone>(pub T);

impl<T> Deref for Base64Field<T>
    where T: StorageValue + Clone
{
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T> Serialize for Base64Field<T>
    where T: StorageValue + Clone
{
    fn serialize<S>(&self, ser: &mut S) -> Result<(), S::Error>
        where S: Serializer
    {
        let vec_bytes = self.0.clone().serialize(); 
        ser.serialize_str(&(encode(&vec_bytes)))
    }
}

struct Base64Visitor<T>
    where T: StorageValue
{
    _p: PhantomData<T>,
}

impl<T> Visitor for Base64Visitor<T>
    where T: StorageValue + Clone
{
    type Value = Base64Field<T>;

    fn visit_str<E>(&mut self, s: &str) -> Result<Base64Field<T>, E>
        where E: de::Error
    {
        
        let vec_bytes = decode(s).map_err(|_| de::Error::custom("Invalid base64 representation"))?; 
        let v = T::deserialize(vec_bytes);
        Ok(Base64Field(v))
    }
}

impl<T> Deserialize for Base64Field<T>
    where T: StorageValue + Clone
{
    fn deserialize<D>(deserializer: &mut D) -> Result<Self, D::Error>
        where D: Deserializer
    {
        deserializer.deserialize_str(Base64Visitor { _p: PhantomData })
    }
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
        *self
    }
}

// impl StorageValue for RawMessage {
//     fn serialize(self) -> Vec<u8> {
//         self.as_ref().as_ref().to_vec()
//     }

//     fn deserialize(v: Vec<u8>) -> Self {
//         Arc::new(MessageBuffer::from_vec(v))
//     }

//     fn hash(&self) -> Hash {
//         self.as_ref().hash()
//     }
// }

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
        <Self as Message>::hash(self)
    }
}

impl<AppTx> StorageValue for AnyTx<AppTx>
    where AppTx: Message
{
    fn serialize(self) -> Vec<u8> {
        self.raw().as_ref().as_ref().to_vec()
    }

    fn deserialize(v: Vec<u8>) -> Self {
        let raw = Arc::new(MessageBuffer::from_vec(v));
        Self::from_raw(raw).unwrap()
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
