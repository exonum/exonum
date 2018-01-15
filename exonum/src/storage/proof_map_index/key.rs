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

use std::cmp::min;

use crypto::{Hash, PublicKey, HASH_SIZE};
use super::super::StorageKey;

pub const BRANCH_KEY_PREFIX: u8 = 00;
pub const LEAF_KEY_PREFIX: u8 = 01;

/// Size in bytes of the `ProofMapKey`.
pub const KEY_SIZE: usize = HASH_SIZE;
pub const DB_KEY_SIZE: usize = KEY_SIZE + 2;

/// A trait that defines a subset of storage key types which are suitable for use with
/// `ProofMapIndex`.
///
/// The size of the keys must be exactly 32 bytes and the keys must have a uniform distribution.
pub trait ProofMapKey: StorageKey {}

impl ProofMapKey for Hash {}
impl ProofMapKey for PublicKey {}
impl ProofMapKey for [u8; KEY_SIZE] {}

impl StorageKey for [u8; KEY_SIZE] {
    fn size(&self) -> usize {
        KEY_SIZE
    }

    fn write(&self, buffer: &mut [u8]) {
        buffer.copy_from_slice(self.as_ref())
    }

    fn read(buffer: &[u8]) -> Self {
        let mut value = [0; KEY_SIZE];
        value.copy_from_slice(buffer);
        value
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ChildKind {
    Left,
    Right,
}

impl ::std::ops::Not for ChildKind {
    type Output = ChildKind;

    fn not(self) -> ChildKind {
        match self {
            ChildKind::Left => ChildKind::Right,
            ChildKind::Right => ChildKind::Left,
        }
    }
}

/// A structure that represents paths to the any kinds of `ProofMapIndex` nodes.
#[derive(Copy, Clone)]
pub struct DBKey {
    bytes: [u8; DB_KEY_SIZE],
    start: u16,
}

impl DBKey {
    /// Create a path from the given key.
    pub fn new<K: ProofMapKey>(key: &K) -> DBKey {
        debug_assert_eq!(key.size(), KEY_SIZE);

        let mut data = [0; DB_KEY_SIZE];
        data[0] = LEAF_KEY_PREFIX;
        key.write(&mut data[1..KEY_SIZE + 1]);
        data[KEY_SIZE + 1] = 0;
        DBKey::from_raw(data)
    }

    /// Shows the type of path.
    pub fn is_leaf(&self) -> bool {
        self.bytes[0] == LEAF_KEY_PREFIX
    }

    /// Returns the byte representation of `DBKey`.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Constructs the `DBKey` from raw bytes.
    fn from_raw(raw: [u8; DB_KEY_SIZE]) -> DBKey {
        DBKey {
            bytes: raw,
            start: 0,
        }
    }

    /// Sets the right border of the bit range.
    fn set_end(&mut self, end: u16) {
        let max_len = (KEY_SIZE * 8) as u16;
        assert!(end <= max_len);
        // Update DBKey kind and right bound.
        if end == max_len {
            self.bytes[DB_KEY_SIZE - 1] = 0;
            self.bytes[0] = LEAF_KEY_PREFIX;
        } else {
            self.bytes[0] = BRANCH_KEY_PREFIX;
            self.bytes[DB_KEY_SIZE - 1] = end as u8;
        };
    }
}

/// The bits representation of the `DBKey`.
pub trait KeyBitRange {
    /// Returns the left border of the range.
    fn start(&self) -> u16;
    /// Returns the right border of the range.
    fn end(&self) -> u16;
    /// Returns length in bits of the range.
    fn len(&self) -> u16 {
        self.end() - self.start()
    }
    /// Returns true if the range has zero length.
    fn is_empty(&self) -> bool {
        self.end() == self.start()
    }
    /// Get bit at position `idx`.
    fn bit(&self, idx: u16) -> ChildKind;
    /// Returns the new `DBKey` with the given left border.
    fn start_from(&self, idx: u16) -> Self;
    /// Shortens this DBKey to the specified length.
    fn prefix(&self, pos: u16) -> Self;
    /// Return object which represents a view on to this slice (further) offset by `i` bits.
    fn suffix(&self, pos: u16) -> Self;
    /// Returns true if we starts with the same prefix at the whole of `Other`
    fn starts_with(&self, other: &Self) -> bool {
        self.common_prefix(other) == other.len()
    }
    /// Returns how many bits at the beginning matches with `other`
    fn common_prefix(&self, other: &Self) -> u16;
    /// Returns the raw bytes of the key.
    fn raw_key(&self) -> &[u8];
}

impl KeyBitRange for DBKey {
    fn start(&self) -> u16 {
        self.start
    }

    fn end(&self) -> u16 {
        if self.is_leaf() {
            KEY_SIZE as u16 * 8
        } else {
            u16::from(self.bytes[DB_KEY_SIZE - 1])
        }
    }

    fn bit(&self, idx: u16) -> ChildKind {
        debug_assert!(self.start() + idx < self.end());

        let pos = self.start() + idx;
        let chunk = self.raw_key()[(pos / 8) as usize];
        let bit = pos % 8;
        let value = (1 << bit) & chunk;
        if value != 0 {
            ChildKind::Right
        } else {
            ChildKind::Left
        }
    }

    fn start_from(&self, start: u16) -> Self {
        debug_assert!(start <= self.end());

        let mut key = DBKey::from_raw(self.bytes);
        key.start = start;
        key
    }

    fn prefix(&self, pos: u16) -> Self {
        debug_assert!(self.start() + pos <= self.raw_key().len() as u16 * 8);

        let mut key = DBKey::from_raw(self.bytes);
        key.start = self.start;
        key.set_end(self.start + pos);
        key
    }

    fn suffix(&self, pos: u16) -> Self {
        debug_assert!(self.start() + pos <= self.end());

        let mut key = DBKey::from_raw(self.bytes);
        key.start = self.start + pos;
        key
    }

    fn common_prefix(&self, other: &Self) -> u16 {
        // We assume that all slices created from byte arrays with the same length
        if self.start() != other.start() {
            0
        } else {
            let from = self.start() / 8;
            let to = min((self.end() + 7) / 8, (other.end() + 7) / 8);
            let max_len = min(self.len(), other.len());

            for i in from..to {
                let x = self.raw_key()[i as usize] ^ other.raw_key()[i as usize];
                if x != 0 {
                    let tail = x.trailing_zeros() as u16;
                    return min(i * 8 + tail - self.start(), max_len);
                }
            }

            max_len
        }
    }

    fn raw_key(&self) -> &[u8] {
        &self.bytes[1..KEY_SIZE + 1]
    }
}

impl PartialEq for DBKey {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len() && self.starts_with(other)
    }
}

impl ::std::fmt::Debug for DBKey {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        let mut bits = String::with_capacity(KEY_SIZE * 8);
        for byte in 0..self.raw_key().len() {
            let chunk = self.raw_key()[byte];
            for bit in (0..8).rev() {
                let i = (byte * 8 + bit) as u16;
                if i < self.start() || i >= self.end() {
                    bits.push('_');
                } else {
                    bits.push(if (1 << bit) & chunk == 0 { '0' } else { '1' });
                }
            }
            bits.push('|');
        }

        f.debug_struct("DBKeyBitRange")
            .field("start", &self.start())
            .field("end", &self.end())
            .field("bits", &bits)
            .finish()
    }
}

impl StorageKey for DBKey {
    fn size(&self) -> usize {
        DB_KEY_SIZE
    }

    fn write(&self, buffer: &mut [u8]) {
        buffer.copy_from_slice(&self.bytes);
        // Cut of the bits that lie to the right of the end.
        if !self.is_leaf() {
            let right = (self.end() as usize + 7) / 8;
            if self.end() % 8 != 0 {
                buffer[right] &= !(255u8 << (self.end() % 8));
            }
            for i in buffer.iter_mut().take(KEY_SIZE + 1).skip(right + 1) {
                *i = 0
            }
        }
    }

    fn read(buffer: &[u8]) -> Self {
        debug_assert_eq!(buffer.len(), DB_KEY_SIZE);
        let mut data = [0; DB_KEY_SIZE];
        data.copy_from_slice(&buffer);
        DBKey::from_raw(data)
    }
}

#[test]
fn test_dbkey_storage_key_leaf() {
    let key = DBKey::new(&[250; 32]);
    let mut buf = vec![0; DB_KEY_SIZE];
    key.write(&mut buf);
    let key2 = DBKey::read(&buf);

    assert_eq!(buf[0], LEAF_KEY_PREFIX);
    assert_eq!(buf[33], 0);
    assert_eq!(&buf[1..33], &[250; 32]);
    assert_eq!(key2, key);
}

#[test]
fn test_dbkey_storage_key_branch() {
    let mut key = DBKey::new(&[255u8; 32]);
    key = key.prefix(11);
    key = key.suffix(5);

    let mut buf = vec![0; DB_KEY_SIZE];
    key.write(&mut buf);
    let mut key2 = DBKey::read(&buf);
    key2.start = 5;

    assert_eq!(buf[0], BRANCH_KEY_PREFIX);
    assert_eq!(buf[33], 11);
    assert_eq!(&buf[1..3], &[255, 7]);
    assert_eq!(&buf[3..33], &[0; 30]);
    assert_eq!(key2, key);
}

#[test]
fn test_dbkey_suffix() {
    let b = DBKey::from_raw(*b"\x00\x01\x02\xFF\x0C0000000000000000000000000000\x20");

    println!("{:?}", b);

    assert_eq!(b.len(), 32);
    assert_eq!(b.bit(0), ChildKind::Right);
    assert_eq!(b.bit(7), ChildKind::Left);
    assert_eq!(b.bit(8), ChildKind::Left);
    assert_eq!(b.bit(9), ChildKind::Right);
    assert_eq!(b.bit(15), ChildKind::Left);
    assert_eq!(b.bit(16), ChildKind::Right);
    assert_eq!(b.bit(20), ChildKind::Right);
    assert_eq!(b.bit(23), ChildKind::Right);
    assert_eq!(b.bit(26), ChildKind::Right);
    assert_eq!(b.bit(27), ChildKind::Right);
    assert_eq!(b.bit(31), ChildKind::Left);
    let b2 = b.suffix(8);
    assert_eq!(b2.len(), 24);
    assert_eq!(b2.bit(0), ChildKind::Left);
    assert_eq!(b2.bit(1), ChildKind::Right);
    assert_eq!(b2.bit(7), ChildKind::Left);
    assert_eq!(b2.bit(12), ChildKind::Right);
    assert_eq!(b2.bit(15), ChildKind::Right);
    let b3 = b2.suffix(24);
    assert_eq!(b3.len(), 0);
    let b4 = b.suffix(1);
    assert_eq!(b4.bit(6), ChildKind::Left);
    assert_eq!(b4.bit(7), ChildKind::Left);
    assert_eq!(b4.bit(8), ChildKind::Right);
}

#[test]
fn test_dbkey_prefix() {
    let b = DBKey::from_raw(*b"\x00\x83wertyuiopasdfghjklzxcvbnm123456\x08");
    assert_eq!(b.len(), 8);
    assert_eq!(b.prefix(1).bit(0), ChildKind::Right);
    assert_eq!(b.prefix(1).len(), 1);
}

#[test]
fn test_dbkey_len() {
    let b = DBKey::from_raw(*b"\x01qwertyuiopasdfghjklzxcvbnm123456\x00");
    assert_eq!(b.len(), 256);
}

#[test]
#[should_panic(expected = "self.start() + idx < self.end()")]
fn test_dbkey_at_overflow() {
    let b = DBKey::from_raw(*b"\x00qwertyuiopasdfghjklzxcvbnm123456\x0F");
    b.bit(32);
}

#[test]
#[should_panic(expected = "self.start() + pos <= self.end()")]
fn test_dbkey_suffix_overflow() {
    let b = DBKey::from_raw(*b"\x00qwertyuiopasdfghjklzxcvbnm123456\xFF");
    assert_eq!(b"\x01qwertyuiopasdfghjklzxcvbnm123456\x00".len(), 34);
    b.suffix(255).suffix(2);
}

#[test]
#[should_panic(expected = "self.start() + idx < self.end()")]
fn test_dbkey_suffix_bit_overflow() {
    let b = DBKey::from_raw(*b"\x00qwertyuiopasdfghjklzxcvbnm123456\xFF");
    b.suffix(1).bit(255);
}

#[test]
fn test_dbkey_common_prefix() {
    let b1 = DBKey::from_raw(*b"\x01abcd0000000000000000000000000000\x00");
    let b2 = DBKey::from_raw(*b"\x01abef0000000000000000000000000000\x00");
    assert_eq!(b1.common_prefix(&b1), 256);
    let c = b1.common_prefix(&b2);
    assert_eq!(c, 17);
    let c = b2.common_prefix(&b1);
    assert_eq!(c, 17);
    let b1 = b1.suffix(9);
    let b2 = b2.suffix(9);
    let c = b1.common_prefix(&b2);
    assert_eq!(c, 8);
    let b3 = DBKey::from_raw(*b"\x01\xFF0000000000000000000000000000000\x00");
    let b4 = DBKey::from_raw(*b"\x01\xF70000000000000000000000000000000\x00");
    assert_eq!(b3.common_prefix(&b4), 3);
    assert_eq!(b4.common_prefix(&b3), 3);
    assert_eq!(b3.common_prefix(&b3), 256);
    let b3 = b3.suffix(30);
    assert_eq!(b3.common_prefix(&b3), 226);
    let b3 = b3.prefix(200);
    assert_eq!(b3.common_prefix(&b3), 200);
    let b5 = DBKey::from_raw(*b"\x01\xF00000000000000000000000000000000\x00");
    assert_eq!(b5.prefix(0).common_prefix(&b3), 0);
}

#[test]
fn test_dbkey_is_leaf() {
    let b = DBKey::from_raw(*b"\x01qwertyuiopasdfghjklzxcvbnm123456\x00");
    assert_eq!(b.len(), 256);
    println!("{:?}", b.suffix(4));
    assert_eq!(b.suffix(4).is_leaf(), true);
    assert_eq!(b.suffix(8).is_leaf(), true);
    assert_eq!(b.suffix(250).is_leaf(), true);
    assert_eq!(b.prefix(16).is_leaf(), false);
}

#[test]
fn test_dbkey_is_branch() {
    let b = DBKey::from_raw(*b"\x00qwertyuiopasdfghjklzxcvbnm123456\xFF");
    assert_eq!(b.len(), 255);
    assert_eq!(b.is_leaf(), false);
}
