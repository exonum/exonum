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

use crypto::{Hash, HashStream, PublicKey, HASH_SIZE};

use super::super::{StorageKey, StorageValue};

pub const BRANCH_KEY_PREFIX: u8 = 00;
pub const LEAF_KEY_PREFIX: u8 = 01;

/// Size in bytes of the `ProofMapKey`.
pub const KEY_SIZE: usize = HASH_SIZE;
pub const DB_KEY_SIZE: usize = KEY_SIZE + 2;

/// A trait that defines a subset of storage key types which are suitable for use with
/// [`ProofMapIndex`].
///
/// The size of the keys must be exactly [`PROOF_MAP_KEY_SIZE`] bytes and the keys must have
/// a uniform distribution.
///
/// [`ProofMapIndex`]: struct.ProofMapIndex.html
/// [`PROOF_MAP_KEY_SIZE`]: constant.PROOF_MAP_KEY_SIZE.html
pub trait ProofMapKey
where
    Self::Output: ProofMapKey,
{
    /// The type of keys as read from the database. `Output` is not necessarily
    /// equal to `Self`, which provides flexibility for [`HashedKey`]s and similar cases
    /// where the key cannot be uniquely restored from the database.
    ///
    /// [`HashedKey`]: trait.HashedKey.html
    type Output;

    /// Writes this key into a byte buffer. The buffer is guaranteed to have size
    /// [`PROOF_MAP_KEY_SIZE`].
    ///
    /// [`PROOF_MAP_KEY_SIZE`]: constant.PROOF_MAP_KEY_SIZE.html
    fn write_key(&self, &mut [u8]);

    /// Reads this key from the buffer.
    fn read_key(&[u8]) -> Self::Output;
}

/// A trait denoting that a certain storage value is suitable for use as a key for
/// [`ProofMapIndex`] after hashing.
///
/// **Warning:** The implementation of the [`ProofMapKey.write_key()`] method provided
/// by this trait is not efficient; it calculates the hash anew on each call.
///
/// # Example
///
/// ```
/// # #[macro_use] extern crate exonum;
/// # use exonum::storage::{MemoryDB, Database, ProofMapIndex, HashedKey};
/// encoding_struct!{
///     struct Point {
///         const SIZE = 8;
///         field x: i32 [0 => 4]
///         field y: i32 [4 => 8]
///     }
/// }
///
/// impl HashedKey for Point {}
///
/// # fn main() {
/// let mut fork = { let db = MemoryDB::new(); db.fork() };
/// let mut map = ProofMapIndex::new("index", &mut fork);
/// map.put(&Point::new(3, -4), 5u32);
/// assert_eq!(map.get(&Point::new(3, -4)), Some(5));
/// assert_eq!(map.get(&Point::new(3, 4)), None);
/// # }
/// ```
///
/// [`ProofMapIndex`]: struct.ProofMapIndex.html
/// [`ProofMapKey.write_key()`]: trait.ProofMapKey.html#tymethod.write_key
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

/// Bit slice type used internally to serialize [`ProofMapKey`]s. A single slice can contain
/// from 1 to [`PROOF_MAP_KEY_SIZE`]`* 8` bits.
///
/// # JSON serialization
///
/// Serialized as a string of `'0'` and `'1'` chars, corresponding exactly to bits in the slice.
///
/// ```
/// # extern crate exonum;
/// extern crate serde_json;
/// use exonum::crypto::Hash;
/// # use exonum::storage::proof_map_index::ProofMapDBKey;
///
/// # fn main() {
/// let key = ProofMapDBKey::leaf(&Hash::default()).truncate(3);
/// assert_eq!(serde_json::to_string(&key).unwrap(), "\"000\"");
/// assert_eq!(
///     serde_json::from_str::<ProofMapDBKey>("\"101010\"").unwrap(),
///     ProofMapDBKey::leaf(&[0b10101000; 32]).truncate(6)
/// );
/// # }
/// ```
///
/// [`ProofMapKey`]: trait.ProofMapKey.html
/// [`PROOF_MAP_KEY_SIZE`]: constant.PROOF_MAP_KEY_SIZE.html
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

    #[doc(hidden)]
    pub fn hashable_prefix(&self, prefix_len: u16) -> DBKeyPrefix {
        debug_assert_eq!(self.from, 0);
        debug_assert!(
            prefix_len <= self.len(),
            "Attempted to extract prefix with length {} from key with length {}",
            prefix_len,
            self.len()
        );
        DBKeyPrefix {
            parent: self,
            prefix_len: prefix_len,
        }
    }
}

#[doc(hidden)]
#[derive(Debug)]
pub struct DBKeyPrefix<'a> {
    parent: &'a DBKey,
    prefix_len: u16,
}

#[doc(hidden)]
impl<'a> DBKeyPrefix<'a> {
    pub fn truncate(&mut self, new_len: u16) {
        debug_assert!(new_len <= self.prefix_len);
        self.prefix_len = new_len;
    }

    pub fn hash_to(&self, stream: HashStream) -> HashStream {
        let mut buffer = [0u8; DB_KEY_SIZE];
        
        if self.prefix_len == (KEY_SIZE * 8) as u16 {
            buffer[0] = LEAF_KEY_PREFIX;
            buffer[1..KEY_SIZE + 1].copy_from_slice(&self.parent.data);
            buffer[KEY_SIZE + 1] = 0;
        } else {
            buffer[0] = BRANCH_KEY_PREFIX;
            let right = (self.prefix_len as usize + 7) / 8;
            buffer[1..right + 1].copy_from_slice(&self.parent.data[0..right]);
            if self.prefix_len % 8 != 0 {
                buffer[right] &= !(255u8 >> (self.prefix_len % 8));
            }
            buffer[KEY_SIZE + 1] = self.prefix_len as u8;
        }
        
        stream.update(&buffer)
    }
}

#[cfg(test)]
mod dbkeyprefix_tests {
    extern crate rand;

    use crypto::hash;
    use rand::Rng;
    use super::*;

    #[test]
    fn test_leaf_key() {
        let key = DBKey::leaf(&[1; 32]);
        let prefix = key.hashable_prefix(256);
        assert_eq!(
            hash(key.as_bytes().as_ref()),
            prefix.hash_to(HashStream::new()).hash()
        );
    }

    #[test]
    fn test_nonleaf_prefixes() {
        let key = DBKey::leaf(&[42; 32]);
        for i in 0..256 {
            let prefix = key.hashable_prefix(i);
            assert_eq!(
                hash(key.truncate(i).as_bytes().as_ref()),
                prefix.hash_to(HashStream::new()).hash()
            );
        }
    }

    #[test]
    fn test_fuzz_prefixes() {
        let mut rng = rand::thread_rng();
        for _ in 0..32 {
            let mut bytes = [0u8; 32];
            rng.fill_bytes(&mut bytes);

            let key = DBKey::leaf(&bytes);
            for i in 0..256 {
                let prefix = key.hashable_prefix(i);
                assert_eq!(
                    hash(key.truncate(i).as_bytes().as_ref()),
                    prefix.hash_to(HashStream::new()).hash()
                );
            }
        }
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
                i if i < 8 => !(255u8 >> i),
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
