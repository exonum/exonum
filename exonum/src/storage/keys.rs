//! A definition of `StorageKey` trait and implementations for common types.
use byteorder::{ByteOrder, BigEndian};
use crypto::{Hash, PublicKey, HASH_SIZE, PUBLIC_KEY_LENGTH};


/// A trait that define serialization of corresponding types as storage keys.
///
/// Since internally the keys are sorted in a serialized form, the big-endian encoding is used.
pub trait StorageKey {
    /// Returns the size of serialized key in bytes.
    fn size(&self) -> usize;

    /// Serialize a key into the specified buffer of bytes.
    ///
    /// The size of the buffer is guaranteed equally to the precalculated size
    /// of the serialized key.
    // TODO: should be unsafe?
    fn write(&self, buffer: &mut [u8]);

    /// Deserialize a key from the specified buffer of bytes.
    // TODO: should be unsafe?
    fn read(buffer: &[u8]) -> Self;
}

impl StorageKey for () {
    fn size(&self) -> usize {
        0
    }

    fn write(&self, _buffer: &mut [u8]) {
        // no-op
    }

    fn read(_buffer: &[u8]) -> Self {
        ()
    }
}

impl StorageKey for u8 {
    fn size(&self) -> usize {
        1
    }

    fn write(&self, buffer: &mut [u8]) {
        buffer[0] = *self
    }

    fn read(buffer: &[u8]) -> Self {
        buffer[0]
    }
}

impl StorageKey for u16 {
    fn size(&self) -> usize {
        2
    }

    fn write(&self, buffer: &mut [u8]) {
        BigEndian::write_u16(buffer, *self)
    }

    fn read(buffer: &[u8]) -> Self {
        BigEndian::read_u16(buffer)
    }
}

impl StorageKey for u32 {
    fn size(&self) -> usize {
        4
    }

    fn write(&self, buffer: &mut [u8]) {
        BigEndian::write_u32(buffer, *self)
    }

    fn read(buffer: &[u8]) -> Self {
        BigEndian::read_u32(buffer)
    }
}

impl StorageKey for u64 {
    fn size(&self) -> usize {
        8
    }

    fn write(&self, buffer: &mut [u8]) {
        BigEndian::write_u64(buffer, *self)
    }

    fn read(buffer: &[u8]) -> Self {
        BigEndian::read_u64(buffer)
    }
}

impl StorageKey for i8 {
    fn size(&self) -> usize {
        1
    }

    fn write(&self, buffer: &mut [u8]) {
        buffer[0] = *self as u8
    }

    fn read(buffer: &[u8]) -> Self {
        buffer[0] as i8
    }
}

impl StorageKey for i16 {
    fn size(&self) -> usize {
        2
    }

    fn write(&self, buffer: &mut [u8]) {
        BigEndian::write_i16(buffer, *self)
    }

    fn read(buffer: &[u8]) -> Self {
        BigEndian::read_i16(buffer)
    }
}

impl StorageKey for i32 {
    fn size(&self) -> usize {
        4
    }

    fn write(&self, buffer: &mut [u8]) {
        BigEndian::write_i32(buffer, *self)
    }

    fn read(buffer: &[u8]) -> Self {
        BigEndian::read_i32(buffer)
    }
}

impl StorageKey for i64 {
    fn size(&self) -> usize {
        8
    }

    fn write(&self, buffer: &mut [u8]) {
        BigEndian::write_i64(buffer, *self)
    }

    fn read(buffer: &[u8]) -> Self {
        BigEndian::read_i64(buffer)
    }
}

impl StorageKey for Hash {
    fn size(&self) -> usize {
        HASH_SIZE
    }

    fn write(&self, buffer: &mut [u8]) {
        buffer.copy_from_slice(self.as_ref())
    }

    fn read(buffer: &[u8]) -> Self {
        Hash::from_slice(buffer).unwrap()
    }
}

impl StorageKey for PublicKey {
    fn size(&self) -> usize {
        PUBLIC_KEY_LENGTH
    }

    fn write(&self, buffer: &mut [u8]) {
        buffer.copy_from_slice(self.as_ref())
    }

    fn read(buffer: &[u8]) -> Self {
        PublicKey::from_slice(buffer).unwrap()
    }
}

impl StorageKey for Vec<u8> {
    fn size(&self) -> usize {
        self.len()
    }

    fn write(&self, buffer: &mut [u8]) {
        buffer.copy_from_slice(self)
    }

    fn read(buffer: &[u8]) -> Self {
        buffer.to_vec()
    }
}

impl StorageKey for String {
    fn size(&self) -> usize {
        self.len()
    }

    fn write(&self, buffer: &mut [u8]) {
        buffer.copy_from_slice(self.as_bytes())
    }

    fn read(buffer: &[u8]) -> Self {
        unsafe { ::std::str::from_utf8_unchecked(buffer).to_string() }
    }
}
