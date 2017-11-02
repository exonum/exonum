// Copyright 2017 The Exonum Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! A definition of `StorageKey` trait and implementations for common types.
use byteorder::{ByteOrder, BigEndian};
use crypto::{Hash, PublicKey, HASH_SIZE, PUBLIC_KEY_LENGTH};


/// A trait that defines serialization of corresponding types as storage keys.
///
/// Since internally the keys are sorted in a serialized form, the big-endian encoding is used.
///
/// # Examples
///
/// Implementing `StorageKey` for the type:
///
/// ```
/// # extern crate exonum;
/// # extern crate byteorder;
///
/// use std::mem;
///
/// use exonum::storage::StorageKey;
/// use byteorder::{LittleEndian, ByteOrder};
///
/// struct Key {
///     a: i16,
///     b: u32,
/// }
///
/// impl StorageKey for Key {
///     fn size(&self) -> usize {
///         mem::size_of_val(&self.a) + mem::size_of_val(&self.b)
///     }
///
///     fn write(&self, buffer: &mut [u8]) {
///         LittleEndian::write_i16(&mut buffer[0..2], self.a);
///         LittleEndian::write_u32(&mut buffer[2..6], self.b);
///     }
///
///     fn read(buffer: &[u8]) -> Self {
///         let a = LittleEndian::read_i16(&buffer[0..2]);
///         let b = LittleEndian::read_u32(&buffer[2..6]);
///         Key { a, b }
///     }
/// }
/// # fn main() {}
/// ```
pub trait StorageKey {
    /// Returns the size of the serialized key in bytes.
    fn size(&self) -> usize;

    /// Serialize a key into the specified buffer of bytes.
    ///
    /// The caller must guarantee that the size of the buffer is equal to the precalculated size
    /// of the serialized key.
    // TODO: should be unsafe (ECR-174)?
    fn write(&self, buffer: &mut [u8]);

    /// Deserialize a key from the specified buffer of bytes.
    // TODO: should be unsafe (ECR-174)?
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
