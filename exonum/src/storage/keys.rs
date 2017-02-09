use byteorder::{ByteOrder, BigEndian};
use ::crypto::{Hash, PublicKey};


pub trait StorageKey {
    fn size() -> usize;
    fn write(&self, buffer: &mut Vec<u8>);
    fn read(buffer: &[u8]) -> Self;
}

pub struct VoidKey;

impl StorageKey for VoidKey {
    fn size() -> usize {
        0
    }

    fn write(&self, _buffer: &mut Vec<u8>) {
        // no-op
    }

    fn read(_buffer: &[u8]) -> Self {
        VoidKey
    }
}

impl StorageKey for u8 {
    fn size() -> usize {
        1
    }

    fn write(&self, buffer: &mut Vec<u8>) {
        buffer[0] = *self
    }

    fn read(buffer: &[u8]) -> Self {
        buffer[0]
    }
}

impl StorageKey for u16 {
    fn size() -> usize {
        2
    }

    fn write(&self, buffer: &mut Vec<u8>) {
        BigEndian::write_u16(buffer, *self)
    }

    fn read(buffer: &[u8]) -> Self {
        BigEndian::read_u16(buffer)
    }
}

impl StorageKey for u32 {
    fn size() -> usize {
        4
    }

    fn write(&self, buffer: &mut Vec<u8>) {
        BigEndian::write_u32(buffer, *self)
    }

    fn read(buffer: &[u8]) -> Self {
        BigEndian::read_u32(buffer)
    }
}

impl StorageKey for u64 {
    fn size() -> usize {
        8
    }

    fn write(&self, buffer: &mut Vec<u8>) {
        BigEndian::write_u64(buffer, *self)
    }

    fn read(buffer: &[u8]) -> Self {
        BigEndian::read_u64(buffer)
    }
}

impl StorageKey for Hash {
    fn size() -> usize {
        32
    }

    fn write(&self, buffer: &mut Vec<u8>) {
        buffer.copy_from_slice(self.as_ref())
    }

    fn read(buffer: &[u8]) -> Self {
        Hash::from_slice(buffer).unwrap()
    }
}

impl StorageKey for PublicKey {
    fn size() -> usize {
        32
    }

    fn write(&self, buffer: &mut Vec<u8>) {
        buffer.copy_from_slice(self.as_ref())
    }

    fn read(buffer: &[u8]) -> Self {
        PublicKey::from_slice(buffer).unwrap()
    }
}


// FIXME: dirty hack for special for patricia merkle tree db keys, need to remove this implementation
impl StorageKey for Vec<u8> {
    fn size() -> usize {
        34
    }

    fn write(&self, buffer: &mut Vec<u8>) {
        assert_eq!(self.len(), Self::size());
        buffer.copy_from_slice(self)
    }

    fn read(buffer: &[u8]) -> Self {
        assert_eq!(buffer.len(), Self::size());
        buffer.to_vec()
    }
}
