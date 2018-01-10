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
use std::fmt::Write;

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

/// A struct that represents a bit slices of the proof map keys.
#[derive(Clone)]
pub struct DBKey {
    data: [u8; KEY_SIZE],
    from: u16,
    to: u16,
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

impl DBKey {
    /// Create a new bit slice from the given binary data.
    pub fn leaf<K: ProofMapKey>(key: &K) -> DBKey {
        debug_assert_eq!(key.size(), KEY_SIZE);

        let mut data = [0; KEY_SIZE];
        key.write(&mut data);
        DBKey {
            data: data,
            from: 0,
            to: (KEY_SIZE * 8) as u16,
        }
    }

    #[doc(hidden)]
    pub fn from(&self) -> u16 {
        self.from
    }

    // TODO: terrible hack, try to remove this (ECR-22)
    #[doc(hidden)]
    pub fn set_from(&mut self, from: u16) {
        self.from = from
    }

    #[doc(hidden)]
    pub fn to(&self) -> u16 {
        self.to
    }

    /// Returns length of the `DBKey`.
    pub fn len(&self) -> u16 {
        self.to - self.from
    }

    /// Returns true if `DBKey` has zero length.
    pub fn is_empty(&self) -> bool {
        self.to == self.from
    }

    /// Get bit at position `idx`.
    pub fn get(&self, idx: u16) -> ChildKind {
        debug_assert!(self.from + idx < self.to);

        let pos = self.from + idx;
        let chunk = self.data[(pos / 8) as usize];
        let bit = pos % 8;
        let value = (1 << bit) & chunk;
        if value != 0 {
            ChildKind::Right
        } else {
            ChildKind::Left
        }
    }

    /// Shortens this DBKey to the specified length.
    pub fn prefix(&self, suffix: u16) -> DBKey {
        debug_assert!(self.from + suffix <= self.data.len() as u16 * 8);

        DBKey {
            data: self.data,
            from: self.from,
            to: self.from + suffix,
        }
    }

    /// Return object which represents a view on to this slice (further) offset by `i` bits.
    pub fn suffix(&self, suffix: u16) -> DBKey {
        debug_assert!(self.from + suffix <= self.to);

        DBKey {
            data: self.data,
            from: self.from + suffix,
            to: self.to,
        }
    }

    /// Shortens this DBKey to the specified length.
    pub fn truncate(&self, size: u16) -> DBKey {
        debug_assert!(self.from + size <= self.to);

        DBKey {
            data: self.data,
            from: self.from,
            to: self.from + size,
        }
    }

    /// Returns how many bits at the beginning matches with `other`
    pub fn common_prefix(&self, other: &Self) -> u16 {
        // We assume that all slices created from byte arrays with the same length
        if self.from != other.from {
            0
        } else {
            let mut max_len = min(self.len(), other.len());

            // let from = self.from / 8;
            // let to = min((self.to + 7) / 8, (other.to + 7) / 8);

            // for i in from..to {
            //     let x = self.data[i as usize] ^ other.data[i as usize];
            //     if x != 0 {
            //         let tail = x.leading_zeros() as u16;
            //         max_len = min(i * 8 + tail - self.from, max_len);
            //         break;
            //     }
            // }

            for i in 0..max_len {
                if self.get(i) != other.get(i) {
                    max_len = i;
                    break;
                }
            }
            
            max_len
        }
    }

    /// Returns true if we starts with the same prefix at the whole of `Other`
    pub fn starts_with(&self, other: &Self) -> bool {
        self.common_prefix(other) == other.len()
    }

    /// Returns true if self.to not changed
    pub fn is_leaf(&self) -> bool {
        self.to == (KEY_SIZE * 8) as u16
    }

    // TODO: terrible hack, try to remove this (ECR-22)
    /// Represents `DBKey` as byte array and returns it
    pub fn as_bytes(&self) -> Box<[u8]> {
        let mut buffer = Box::new([0u8; DB_KEY_SIZE as usize]);
        self.write(buffer.as_mut());
        buffer
    }
}

impl AsRef<[u8]> for DBKey {
    fn as_ref(&self) -> &[u8] {
        &self.data
    }
}

impl StorageKey for DBKey {
    fn size(&self) -> usize {
        DB_KEY_SIZE
    }

    fn write(&self, buffer: &mut [u8]) {
        if self.is_leaf() {
            buffer[0] = LEAF_KEY_PREFIX;
            buffer[1..KEY_SIZE + 1].copy_from_slice(&self.data);
            buffer[KEY_SIZE + 1] = 0;
        } else {
            buffer[0] = BRANCH_KEY_PREFIX;
            let right = (self.to as usize + 7) / 8;
            buffer[1..right + 1].copy_from_slice(&self.data[0..right]);
            if self.to % 8 != 0 {
                buffer[right] &= !(255u8 << (self.to % 8));
            }
            for i in buffer.iter_mut().take(KEY_SIZE + 1).skip(right + 1) {
                *i = 0
            }
            buffer[KEY_SIZE + 1] = self.to as u8;
        }
    }

    fn read(buffer: &[u8]) -> Self {
        debug_assert_eq!(buffer.len(), DB_KEY_SIZE);
        let mut data = [0; KEY_SIZE];
        data[..].copy_from_slice(&buffer[1..KEY_SIZE + 1]);
        let to = match buffer[0] {
            LEAF_KEY_PREFIX => KEY_SIZE as u16 * 8,
            BRANCH_KEY_PREFIX => u16::from(buffer[DB_KEY_SIZE - 1]),
            _ => unreachable!("wrong key prefix"),
        };
        DBKey {
            data: data,
            from: 0,
            to: to,
        }
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
        for byte in 0..self.data.len() {
            let chunk = self.data[byte];
            for bit in (0..8).rev() {
                let i = (byte * 8 + bit) as u16;
                match i {
                    _ if i < self.from => write!(&mut bits, "_")?,
                    _ if i >= self.to => write!(&mut bits, "_")?,
                    _ => {
                        write!(
                            &mut bits,
                            "{}",
                            match (1 << bit) & chunk == 0 {
                                true => '0',
                                false => '1',
                            }
                        )?
                    }
                }
            }
            write!(&mut bits, "|")?;
        }

        f.debug_struct("DBKey")
            .field("begin", &self.from)
            .field("end", &self.to)
            .field("bits", &bits)
            .finish()
    }
}

#[test]
fn test_dbkey_suffix() {
    let b = DBKey::read(b"\x00\x01\x02\xFF\x0C0000000000000000000000000000\x20");

    assert_eq!(b.len(), 32);
    assert_eq!(b.get(0), ChildKind::Right);
    assert_eq!(b.get(7), ChildKind::Left);
    assert_eq!(b.get(8), ChildKind::Left);
    assert_eq!(b.get(9), ChildKind::Right);
    assert_eq!(b.get(15), ChildKind::Left);
    assert_eq!(b.get(16), ChildKind::Right);
    assert_eq!(b.get(20), ChildKind::Right);
    assert_eq!(b.get(23), ChildKind::Right);
    assert_eq!(b.get(26), ChildKind::Right);
    assert_eq!(b.get(27), ChildKind::Right);
    assert_eq!(b.get(31), ChildKind::Left);
    let b2 = b.suffix(8);
    assert_eq!(b2.len(), 24);
    assert_eq!(b2.get(0), ChildKind::Left);
    assert_eq!(b2.get(1), ChildKind::Right);
    assert_eq!(b2.get(7), ChildKind::Left);
    assert_eq!(b2.get(12), ChildKind::Right);
    assert_eq!(b2.get(15), ChildKind::Right);
    let b3 = b2.suffix(24);
    assert_eq!(b3.len(), 0);
    let b4 = b.suffix(1);
    assert_eq!(b4.get(6), ChildKind::Left);
    assert_eq!(b4.get(7), ChildKind::Left);
    assert_eq!(b4.get(8), ChildKind::Right);
}

#[test]
fn test_dbkey_truncate() {
    let b = DBKey::read(b"\x00\x83wertyuiopasdfghjklzxcvbnm123456\x08");
    assert_eq!(b.len(), 8);
    assert_eq!(b.truncate(1).get(0), ChildKind::Right);
    assert_eq!(b.truncate(1).len(), 1);
}

#[test]
fn test_dbkey_len() {
    let b = DBKey::read(b"\x01qwertyuiopasdfghjklzxcvbnm123456\x00");
    assert_eq!(b.len(), 256);
}

#[test]
#[should_panic(expected = "self.from + idx < self.to")]
fn test_dbkey_at_overflow() {
    let b = DBKey::read(b"\x00qwertyuiopasdfghjklzxcvbnm123456\x0F");
    b.get(32);
}

#[test]
#[should_panic(expected = "self.from + suffix <= self.to")]
fn test_dbkey_suffix_overflow() {
    let b = DBKey::read(b"\x00qwertyuiopasdfghjklzxcvbnm123456\xFF");
    assert_eq!(b"\x01qwertyuiopasdfghjklzxcvbnm123456\x00".len(), 34);
    b.suffix(255).suffix(2);
}

#[test]
#[should_panic(expected = "self.from + idx < self.to")]
fn test_dbkey_suffix_at_overflow() {
    let b = DBKey::read(b"\x00qwertyuiopasdfghjklzxcvbnm123456\xFF");
    b.suffix(1).get(255);
}

#[test]
fn test_dbkey_common_prefix() {
    let b1 = DBKey::read(b"\x01abcd0000000000000000000000000000\x00");
    let b2 = DBKey::read(b"\x01abef0000000000000000000000000000\x00");
    assert_eq!(b1.common_prefix(&b1), 256);
    let c = b1.common_prefix(&b2);
    assert_eq!(c, 17);
    let c = b2.common_prefix(&b1);
    assert_eq!(c, 17);
    let b1 = b1.suffix(9);
    let b2 = b2.suffix(9);
    let c = b1.common_prefix(&b2);
    assert_eq!(c, 8);
    let b3 = DBKey::read(b"\x01\xFF0000000000000000000000000000000\x00");
    let b4 = DBKey::read(b"\x01\xF70000000000000000000000000000000\x00");
    assert_eq!(b3.common_prefix(&b4), 3);
    assert_eq!(b4.common_prefix(&b3), 3);
    assert_eq!(b3.common_prefix(&b3), 256);
    let b3 = b3.suffix(30);
    assert_eq!(b3.common_prefix(&b3), 226);
    let b3 = b3.truncate(200);
    assert_eq!(b3.common_prefix(&b3), 200);
    let b5 = DBKey::read(b"\x01\xF00000000000000000000000000000000\x00");
    assert_eq!(b5.truncate(0).common_prefix(&b3), 0);
}

#[test]
fn test_dbkey_is_leaf() {
    let b = DBKey::read(b"\x01qwertyuiopasdfghjklzxcvbnm123456\x00");
    assert_eq!(b.len(), 256);
    assert_eq!(b.suffix(4).is_leaf(), true);
    assert_eq!(b.suffix(8).is_leaf(), true);
    assert_eq!(b.suffix(250).is_leaf(), true);
    assert_eq!(b.truncate(16).is_leaf(), false);
}

#[test]
fn test_dbkey_is_branch() {
    let b = DBKey::read(b"\x00qwertyuiopasdfghjklzxcvbnm123456\xFF");
    assert_eq!(b.len(), 255);
    assert_eq!(b.is_leaf(), false);
}
