use std::mem;
use std::sync::Arc;
use base64::{encode_mode, decode_mode, Base64Error, Base64Mode};
use byteorder::{ByteOrder, BigEndian};

use ::crypto::{Hash, hash, HASH_SIZE};
use ::messages::{MessageBuffer, Message, AnyTx};

#[derive(Clone)]
pub struct HeightBytes(pub [u8; 32]);

pub trait StorageValue {
    fn serialize(&self, buf: Vec<u8>) -> Vec<u8>;
    ///to inform caller what capacity is needed for serialize beforehand
    fn len_hint(&self) -> usize {
        0
    } 
    fn deserialize(v: Vec<u8>) -> Self;
    fn hash(&self) -> Hash {
        hash(&self.serialize(Vec::new()))
    }  
}

pub fn repr_stor_val<T: StorageValue>(value: &T) -> String {
    let vec_bytes = value.serialize(Vec::new());
    encode_mode(&vec_bytes, Base64Mode::UrlSafe)
}

pub fn decode_from_b64_string<T: StorageValue>(b64: &str) -> Result<T, Base64Error> {
    let vec_bytes = decode_mode(b64, Base64Mode::UrlSafe)?; 
    Ok(StorageValue::deserialize(vec_bytes))
}

impl StorageValue for u16 {
    fn serialize(&self, mut buf: Vec<u8>) -> Vec<u8> {
        let old_len = buf.len(); 
        let new_len = old_len + mem::size_of::<u16>(); 
        buf.resize(new_len, 0); 

        BigEndian::write_u16(&mut buf[old_len..new_len], *self);  
        buf 
    }

    fn deserialize(v: Vec<u8>) -> Self {
        BigEndian::read_u16(&v)
    }

    fn len_hint(&self) -> usize {
        mem::size_of::<u16>() 
    }
}

impl StorageValue for u32 {
    // TODO: return Cow<[u8]>
    fn serialize(&self, mut buf: Vec<u8>) -> Vec<u8> {
        let old_len = buf.len(); 
        let new_len = old_len + mem::size_of::<u32>(); 
        buf.resize(new_len, 0);
        BigEndian::write_u32(&mut buf[old_len..new_len], *self); 
        buf
    }

    fn deserialize(v: Vec<u8>) -> Self {
        BigEndian::read_u32(&v)
    }

    fn len_hint(&self) -> usize {
        mem::size_of::<u32>() 
    }
}

impl StorageValue for u64 {
    fn serialize(&self, mut buf: Vec<u8>) -> Vec<u8> {
        let old_len = buf.len(); 
        let new_len = old_len + mem::size_of::<u64>(); 
        buf.resize(new_len, 0);
        BigEndian::write_u64(&mut buf[old_len..new_len], *self);
        buf
    }

    fn deserialize(v: Vec<u8>) -> Self {
        BigEndian::read_u64(&v)
    }

    fn len_hint(&self) -> usize {
        mem::size_of::<u64>() 
    }
}

impl StorageValue for i64 {
    fn serialize(&self, mut buf: Vec<u8>) -> Vec<u8> {
        let old_len = buf.len(); 
        let new_len = old_len + mem::size_of::<i64>(); 
        buf.resize(new_len, 0);
        BigEndian::write_i64(&mut buf[old_len..new_len], *self);
        buf
    }

    fn deserialize(v: Vec<u8>) -> Self {
        BigEndian::read_i64(&v)
    }

    fn len_hint(&self) -> usize {
        mem::size_of::<i64>() 
    }
}

impl StorageValue for Hash {
    fn serialize(&self, mut buf: Vec<u8>) -> Vec<u8> {

        let byteslice = self.as_ref();   
        let old_len = buf.len(); 
        let new_len = old_len + byteslice.len(); 
        buf.resize(new_len, 0);
        {
            let part = &mut buf[old_len..new_len]; 
            part.copy_from_slice(byteslice); 
        }
        buf
    }

    fn deserialize(v: Vec<u8>) -> Self {
        Hash::from_slice(&v).unwrap()
    }

    fn len_hint(&self) -> usize {
        HASH_SIZE
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
    fn serialize(&self, mut buf: Vec<u8>) -> Vec<u8> {
        let byteslice = self.raw().as_ref().as_ref();
        let old_len = buf.len(); 
        let new_len = old_len + byteslice.len(); 
        buf.resize(new_len, 0);
        {
            let part = &mut buf[old_len..new_len]; 
            part.copy_from_slice(byteslice); 
        }
        buf
    }

    fn deserialize(v: Vec<u8>) -> Self {
        Message::from_raw(Arc::new(MessageBuffer::from_vec(v))).unwrap()
    }

    fn len_hint(&self) -> usize {
        self.raw().as_ref().as_ref().len() 
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
    
    fn serialize(&self, mut buf: Vec<u8>) -> Vec<u8> {
        let old_len = buf.len(); 
        let new_len = old_len + self.len(); 
        buf.resize(new_len, 0);
        {
            let part = &mut buf[old_len..new_len]; 
            part.copy_from_slice(self); 
        }
        buf
    }

    fn deserialize(v: Vec<u8>) -> Self {
        v
    }

    fn len_hint(&self) -> usize {
        self.len()
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
