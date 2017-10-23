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

use std::cmp::{min, Ordering};

use crypto::{Hash, PublicKey, HASH_SIZE};

use super::super::{StorageKey, StorageValue};

pub const BRANCH_KEY_PREFIX: u8 = 00;
pub const LEAF_KEY_PREFIX: u8 = 01;

/// Size in bytes of the `ProofMapKey`.
pub const KEY_SIZE: usize = HASH_SIZE;
pub const DB_KEY_SIZE: usize = KEY_SIZE + 2;

/// A trait that defines a subset of storage key types which are suitable for use with
/// `ProofMapIndex`.
///
/// The size of the keys must be exactly `KEY_SIZE` bytes and the keys must have
/// a uniform distribution.
pub trait ProofMapKey
where
    Self::Output: ProofMapKey,
{
    /// The type of keys as read from the database. `Output` is not necessarily
    /// equal to `Self`, which provides flexibility for `HashedKey`s and similar cases
    /// where the key cannot be uniquely restored from the database.
    type Output;

    /// Writes this key into a byte buffer. The buffer is guaranteed to have size `KEY_SIZE`.
    fn write_key(&self, &mut [u8]);

    /// Reads this key from the buffer.
    fn read_key(&[u8]) -> Self::Output;
}

/// A trait denoting that a certain storage value is suitable for use as a key for `ProofMapIndex`
/// after hashing.
///
/// **Warning:** The implementation of the `write_key()` method of `ProofMapKey` provided
/// by this trait is not efficient; it calculates the hash anew on each call.
pub trait HashedKey: StorageValue {}

impl<T: HashedKey> ProofMapKey for T {
    type Output = Hash;

    fn write_key(&self, buffer: &mut [u8]) {
        self.hash().write(buffer);
    }

    fn read_key(buffer: &[u8]) -> Hash {
        <Hash as StorageKey>::read(buffer)
    }
}

// TODO: consider removing.
impl ProofMapKey for PublicKey {
    type Output = PublicKey;

    fn write_key(&self, buffer: &mut [u8]) {
        StorageKey::write(self, buffer);
    }

    fn read_key(raw: &[u8]) -> PublicKey {
        <PublicKey as StorageKey>::read(raw)
    }
}

impl ProofMapKey for Hash {
    type Output = Hash;

    fn write_key(&self, buffer: &mut [u8]) {
        StorageKey::write(self, buffer);
    }

    fn read_key(raw: &[u8]) -> Hash {
        <Hash as StorageKey>::read(raw)
    }
}

// TODO: should probably be removed; `[u8; 32]` values are not guaranteed
// to be uniformly distributed.
impl ProofMapKey for [u8; 32] {
    type Output = [u8; 32];

    fn write_key(&self, buffer: &mut [u8]) {
        buffer.copy_from_slice(self);
    }

    fn read_key(raw: &[u8]) -> [u8; 32] {
        let mut value = [0; KEY_SIZE];
        value.copy_from_slice(raw);
        value
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ChildKind {
    Left,
    Right,
}

/// A struct that represents a bit slices of the proof map keys.
#[derive(Clone, Copy)]
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
        let mut data = [0; KEY_SIZE];
        key.write_key(&mut data);
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
        let bit = 7 - pos % 8;
        let value = (1 << bit) & chunk;
        if value != 0 {
            ChildKind::Right
        } else {
            ChildKind::Left
        }
    }

    /// Shortens this DBKey to the specified length.
    pub fn prefix(&self, mid: u16) -> DBKey {
        DBKey {
            data: self.data,
            from: self.from,
            to: self.from + mid,
        }
    }

    /// Return object which represents a view on to this slice (further) offset by `i` bits.
    pub fn suffix(&self, mid: u16) -> DBKey {
        debug_assert!(self.from + mid <= self.to);

        DBKey {
            data: self.data,
            from: self.from + mid,
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

    /// Shortens this `DBKey` to the specified length. Unlike `truncate()`, the transformation
    /// is performed in place.
    pub fn truncate_in_place(&mut self, size: u16) {
        debug_assert!(self.from + size <= self.to);
        self.to = self.from + size;
    }

    /// Returns the number of matching bits with `other` starting from position `from`.
    fn match_len(&self, other: &Self, from: u16) -> u16 {
        let from = from / 8;
        let to = min((self.to + 7) / 8, (other.to + 7) / 8);
        let max_len = min(self.len(), other.len());

        for i in from..to {
            let x = self.data[i as usize] ^ other.data[i as usize];
            if x != 0 {
                let tail = x.leading_zeros() as u16;
                return min(i * 8 + tail - self.from, max_len);
            }
        }
        max_len
    }

    /// Returns how many bits at the beginning matches with `other`
    pub fn common_prefix(&self, other: &Self) -> u16 {
        // We assume that all slices created from byte arrays with the same length
        if self.from != other.from {
            0
        } else {
            self.match_len(other, self.from)
        }
    }

    /// Returns true if we starts with the same prefix at the whole of `Other`
    pub fn starts_with(&self, other: &Self) -> bool {
        self.common_prefix(other) == other.len()
    }

    #[doc(hidden)]
    pub fn matches_from(&self, other: &Self, from: u16) -> bool {
        debug_assert!(from >= self.from);
        self.match_len(other, from) == other.len()
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
                buffer[right] &= !(255u8 >> (self.to % 8));
            }
            for i in buffer.iter_mut().take(KEY_SIZE + 1).skip(right + 1) {
                *i = 0
            }
            buffer[KEY_SIZE + 1] = self.to as u8;
        }
    }

    fn read(buffer: &[u8]) -> Self {
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

impl PartialOrd for DBKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if self.from != other.from {
            None
        } else {
            let from = self.from / 8;
            let to = min(self.to / 8, other.to / 8);

            for i in from..to {
                let ord = self.data[i as usize].cmp(&other.data[i as usize]);
                if ord != Ordering::Equal {
                    return Some(ord);
                }
            }

            let bits = min(self.to - to * 8, other.to - to * 8);
            let mask: u8 = match bits {
                0 => return Some(self.to.cmp(&other.to)),
                1 => 0b_1000_0000,
                2 => 0b_1100_0000,
                3 => 0b_1110_0000,
                4 => 0b_1111_0000,
                5 => 0b_1111_1000,
                6 => 0b_1111_1100,
                7 => 0b_1111_1110,
                _ => unreachable!("Unexpected number of trailing bits in DBKey comparison"),
            };

            // Here, `to < 32`. Indeed, `to == 32` is possible only if `self.to == other.to == 256`,
            // in which case `bits == 0`, which is handled in the match above.
            let ord = (self.data[to as usize] & mask).cmp(&(other.data[to as usize] & mask));
            if ord != Ordering::Equal {
                return Some(ord);
            }

            Some(self.to.cmp(&other.to))
        }
    }
}

impl ::std::fmt::Debug for DBKey {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        write!(f, "DBKey(")?;
        for i in 0..self.len() {
            write!(
                f,
                "{}",
                match self.get(i) {
                    ChildKind::Left => '0',
                    ChildKind::Right => '1',
                }
            )?;
        }
        write!(f, ")")
    }
}
