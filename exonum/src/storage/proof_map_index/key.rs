// Copyright 2018 The Exonum Team
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
use std::ops;

use crypto::{CryptoHash, Hash, PublicKey, HASH_SIZE};
use storage::StorageKey;

pub const BRANCH_KEY_PREFIX: u8 = 0;
pub const LEAF_KEY_PREFIX: u8 = 1;

/// Size in bytes of the `ProofMapKey`.
///
/// Equal to the size of the hash function output (32).
pub const KEY_SIZE: usize = HASH_SIZE;
/// Size in bytes of the `ProofPath`.
pub const PROOF_PATH_SIZE: usize = KEY_SIZE + 2;
/// Position of the byte with kind of the `ProofPath`.
pub const PROOF_PATH_KIND_POS: usize = 0;
/// Position of the byte with total length of the branch.
pub const PROOF_PATH_LEN_POS: usize = KEY_SIZE + 1;

/// A trait that defines a subset of storage key types which are suitable for use with
/// `ProofMapIndex`.
///
/// The size of the keys must be exactly [`PROOF_MAP_KEY_SIZE`] bytes and the keys must have
/// a uniform distribution.
///
/// [`PROOF_MAP_KEY_SIZE`]: constant.PROOF_MAP_KEY_SIZE.html
pub trait ProofMapKey
where
    Self::Output: ProofMapKey,
{
    /// The type of keys as read from the database.
    ///
    /// `Output` is not necessarily equal to `Self`, which provides flexibility
    /// for [`HashedKey`]s and similar cases
    /// where the key cannot be uniquely restored from the database.
    ///
    /// [`HashedKey`]: trait.HashedKey.html
    type Output;

    /// Writes this key into a byte buffer.
    ///
    /// The buffer is guaranteed to have size [`PROOF_MAP_KEY_SIZE`].
    ///
    /// [`PROOF_MAP_KEY_SIZE`]: constant.PROOF_MAP_KEY_SIZE.html
    fn write_key(&self, &mut [u8]);

    /// Reads this key from the buffer.
    fn read_key(&[u8]) -> Self::Output;
}

/// A trait denoting that a certain storage value is suitable for use as a key for
/// `ProofMapIndex` after hashing.
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
///         x: i32,
///         y: i32,
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
pub trait HashedKey: CryptoHash {}

impl<T: HashedKey> ProofMapKey for T {
    type Output = Hash;

    fn write_key(&self, buffer: &mut [u8]) {
        self.hash().write(buffer);
    }

    fn read_key(buffer: &[u8]) -> Hash {
        <Hash as StorageKey>::read(buffer)
    }
}

impl ProofMapKey for PublicKey {
    type Output = Self;

    fn write_key(&self, buffer: &mut [u8]) {
        StorageKey::write(self, buffer);
    }

    fn read_key(raw: &[u8]) -> Self {
        <Self as StorageKey>::read(raw)
    }
}

impl ProofMapKey for Hash {
    type Output = Self;

    fn write_key(&self, buffer: &mut [u8]) {
        StorageKey::write(self, buffer);
    }

    fn read_key(raw: &[u8]) -> Self {
        <Self as StorageKey>::read(raw)
    }
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ChildKind {
    Left,
    Right,
}

impl ops::Not for ChildKind {
    type Output = Self;

    fn not(self) -> Self {
        match self {
            ChildKind::Left => ChildKind::Right,
            ChildKind::Right => ChildKind::Left,
        }
    }
}

/// Bit slice type used internally to serialize `ProofMapKey`s.
///
/// A single slice can contain from 1 to [`PROOF_MAP_KEY_SIZE`]`* 8` bits.
///
/// # Binary representation
///
/// | Position in bytes     | Description                   	                    |
/// |-------------------    |----------------------------------------------         |
/// | 0               	    | `ProofPath` kind: (0 is branch, 1 is leaf)            |
/// | 1..33                 | `ProofMapKey` bytes.    	                            |
/// | 33                    | Total length in bits of `ProofMapKey` for branches.   |
///
/// # JSON serialization
///
/// Serialized as a string of `'0'` and `'1'` chars, corresponding exactly to bits in the slice.
///
/// [`PROOF_MAP_KEY_SIZE`]: constant.PROOF_MAP_KEY_SIZE.html
#[derive(Copy, Clone)]
pub struct ProofPath {
    bytes: [u8; PROOF_PATH_SIZE],
    start: u16,
}

impl ProofPath {
    /// Creates a path from the given key.
    pub fn new<K: ProofMapKey>(key: &K) -> Self {
        let mut data = [0; PROOF_PATH_SIZE];
        data[0] = LEAF_KEY_PREFIX;
        key.write_key(&mut data[1..KEY_SIZE + 1]);
        data[PROOF_PATH_LEN_POS] = 0;
        Self::from_raw(data)
    }

    /// Checks if this is a path to a leaf `ProofMapIndex` node.
    pub fn is_leaf(&self) -> bool {
        self.bytes[0] == LEAF_KEY_PREFIX
    }

    /// Returns the byte representation of contained `ProofMapKey`.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Constructs the `ProofPath` from raw bytes.
    fn from_raw(raw: [u8; PROOF_PATH_SIZE]) -> Self {
        debug_assert!(
            (raw[PROOF_PATH_KIND_POS] != LEAF_KEY_PREFIX) || (raw[PROOF_PATH_LEN_POS] == 0),
            "ProofPath is inconsistent"
        );

        Self {
            bytes: raw,
            start: 0,
        }
    }

    /// Sets the right border of the bit range.
    fn set_end(&mut self, end: Option<u8>) {
        // Updates ProofPath kind and right bound.
        if let Some(pos) = end {
            self.bytes[0] = BRANCH_KEY_PREFIX;
            self.bytes[PROOF_PATH_LEN_POS] = pos as u8;
        } else {
            self.bytes[0] = LEAF_KEY_PREFIX;
            self.bytes[PROOF_PATH_LEN_POS] = 0;
        };
    }
}

/// The bits representation of the `ProofPath`.
pub(crate) trait BitsRange {
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

    /// Gets bit at index `idx`.
    fn bit(&self, idx: u16) -> ChildKind {
        debug_assert!(self.start() + idx < self.end());

        let pos = self.start() + idx;
        let chunk = self.raw_key()[(pos / 8) as usize];
        let bit = pos % 8;
        let value = (1 << bit) & chunk;
        if value == 0 {
            ChildKind::Left
        } else {
            ChildKind::Right
        }
    }

    /// Returns a copy of this bit range with the given left border.
    fn start_from(&self, pos: u16) -> Self;

    /// Returns a copy of this bit range shortened to the specified length.
    fn prefix(&self, len: u16) -> Self;

    /// Returns a copy of this bit range where the start is shifted by the `len`
    /// bits to the right.
    fn suffix(&self, len: u16) -> Self;

    /// Checks if this bit range contains the other bit range as a prefix,
    /// provided that the start positions of both ranges are the same.
    fn starts_with(&self, other: &Self) -> bool {
        self.common_prefix_len(other) == other.len()
    }

    /// Returns the raw bytes of the key.
    fn raw_key(&self) -> &[u8];

    /// Returns the number of matching bits with `other`, where checking bits for equality starts
    /// from the specified position (`from`).
    ///
    /// Bits preceding `from` are not checked and assumed to be equal in both ranges (e.g.,
    /// because they have been checked previously).
    fn match_len(&self, other: &Self, from: u16) -> u16 {
        debug_assert_eq!(self.start(), other.start(), "Misaligned bit ranges");
        debug_assert!(from >= self.start() && from <= self.end());

        let from = from / 8;
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

    /// Checks if this range of bits matches the other one starting from the specified offset.
    fn matches_from(&self, other: &Self, from: u16) -> bool {
        self.match_len(other, from) == other.len()
    }

    /// Returns the length of the common prefix between this and the other range,
    /// provided that they start from the same position.
    /// If start positions differ, returns 0.
    fn common_prefix_len(&self, other: &Self) -> u16 {
        if self.start() == other.start() {
            self.match_len(other, self.start())
        } else {
            0
        }
    }
}

impl BitsRange for ProofPath {
    fn start(&self) -> u16 {
        self.start
    }

    fn end(&self) -> u16 {
        if self.is_leaf() {
            KEY_SIZE as u16 * 8
        } else {
            u16::from(self.bytes[PROOF_PATH_LEN_POS])
        }
    }

    fn start_from(&self, pos: u16) -> Self {
        debug_assert!(pos <= self.end());

        let mut key = Self::from_raw(self.bytes);
        key.start = pos;
        key
    }

    fn prefix(&self, len: u16) -> Self {
        let end = self.start + len;
        let key_len = KEY_SIZE as u16 * 8;
        debug_assert!(end < key_len);

        let mut key = Self::from_raw(self.bytes);
        key.start = self.start;
        key.set_end(Some(end as u8));
        key
    }

    fn suffix(&self, len: u16) -> Self {
        self.start_from(self.start() + len)
    }

    fn raw_key(&self) -> &[u8] {
        &self.bytes[1..KEY_SIZE + 1]
    }
}

impl PartialEq for ProofPath {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len() && self.starts_with(other)
    }
}

impl ::std::fmt::Debug for ProofPath {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        // 8 bits + '|' symbol per byte.
        let mut bits = String::with_capacity(KEY_SIZE * 9);
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

        f.debug_struct("ProofPath")
            .field("start", &self.start())
            .field("end", &self.end())
            .field("bits", &bits)
            .finish()
    }
}

impl StorageKey for ProofPath {
    fn size(&self) -> usize {
        PROOF_PATH_SIZE
    }

    fn write(&self, buffer: &mut [u8]) {
        buffer.copy_from_slice(&self.bytes);
        // Cut of the bits that lie to the right of the end.
        if !self.is_leaf() {
            let right = (self.end() as usize + 7) / 8;
            if self.end() % 8 != 0 {
                buffer[right] &= !(255_u8 << (self.end() % 8));
            }
            for i in buffer.iter_mut().take(KEY_SIZE + 1).skip(right + 1) {
                *i = 0
            }
        }
    }

    fn read(buffer: &[u8]) -> Self::Owned {
        debug_assert_eq!(buffer.len(), PROOF_PATH_SIZE);
        let mut data = [0; PROOF_PATH_SIZE];
        data.copy_from_slice(buffer);
        Self::from_raw(data)
    }
}

impl PartialOrd for ProofPath {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if self.start() != other.start() {
            return None;
        }
        // NB: This check can be moved to "real" code; the code below does not work
        // if `self.start() % 8 != 0` without additional modifications.
        assert_eq!(self.start(), 0);

        let right_bit = min(self.end(), other.end());
        let right = (right_bit as usize + 7) / 8;

        for i in 0..right {
            let (mut self_byte, mut other_byte) = (self.raw_key()[i], other.raw_key()[i]);

            if i + 1 == right && right_bit % 8 != 0 {
                // Cut possible junk after the end of path(s)
                self_byte &= !(255 << (right_bit % 8));
                other_byte &= !(255 << (right_bit % 8));
            }

            // Try to find a first bit index at which this path is greater than the other path
            // (i.e., a bit of this path is 1 and the corresponding bit of the other path
            // is 0), and vice versa. The smaller of these indexes indicates the actual
            // larger path. In turn, the indexes can be found by counting trailing zeros.
            let self_zeros = (self_byte & !other_byte).trailing_zeros();
            let other_zeros = (!self_byte & other_byte).trailing_zeros();

            let cmp = other_zeros.cmp(&self_zeros);
            if cmp != Ordering::Equal {
                return Some(cmp);
            }
        }

        Some(self.end().cmp(&other.end()))
    }
}

#[cfg(test)]
mod tests {
    use rand::{self, Rng};
    use serde_json::{self, Value};

    use super::*;

    /// Creates a random non-leaf, non-empty path.
    fn random_path<T: Rng>(rng: &mut T) -> ProofPath {
        ProofPath::new(&{
            let mut buf = [0; 32];
            rng.fill_bytes(&mut buf);
            buf
        }).prefix(1 + rng.gen::<u16>() % 255)
    }

    #[test]
    fn test_proof_path_serialization() {
        let path = ProofPath::new(&[1; 32]).prefix(3);
        assert_eq!(serde_json::to_value(&path).unwrap(), json!("100"));
        let path: ProofPath = serde_json::from_value(json!("101001")).unwrap();
        assert_eq!(path, ProofPath::new(&[0b00_100101; 32]).prefix(6));

        // Fuzz tests for roundtrip.
        let mut rng = rand::thread_rng();
        for _ in 0..1000 {
            let path = random_path(&mut rng);

            let value = serde_json::to_value(&path).unwrap();
            let other_path: ProofPath = serde_json::from_value(value.clone()).unwrap();
            assert_eq!(other_path, path);

            if let Value::String(s) = value {
                assert_eq!(s.len(), path.len() as usize);
                for (i, byte) in s.bytes().enumerate() {
                    assert_eq!(
                        byte,
                        match path.bit(i as u16) {
                            ChildKind::Left => b'0',
                            ChildKind::Right => b'1',
                        }
                    );
                }
            } else {
                panic!("Incorrect ProofPath serialization, string expected");
            }
        }
    }

    #[test]
    fn test_proof_path_ordering() {
        assert!(ProofPath::new(&[1; 32]) > ProofPath::new(&[254; 32]));
        assert!(ProofPath::new(&[0b0001_0001; 32]) > ProofPath::new(&[0b0010_0001; 32]));
        assert!(ProofPath::new(&[1; 32]) == ProofPath::new(&[1; 32]));
        assert!(ProofPath::new(&[1; 32]).prefix(6) == ProofPath::new(&[129; 32]).prefix(6));
        assert!(ProofPath::new(&[1; 32]).prefix(254) < ProofPath::new(&[1; 32]));

        let mut rng = rand::thread_rng();
        for _ in 0..1000 {
            let (x, y) = (random_path(&mut rng), random_path(&mut rng));
            let x_bits = (0..x.len()).map(|i| x.bit(i));
            let y_bits = (0..y.len()).map(|i| y.bit(i));
            assert_eq!(x.partial_cmp(&y).unwrap(), x_bits.cmp(y_bits));
        }
    }

    #[test]
    fn test_fuzz_match_len() {
        let mut rng = rand::thread_rng();
        for _ in 0..10_000 {
            let (x, y) = (random_path(&mut rng), random_path(&mut rng));
            let min_len = min(x.len(), y.len());
            let start = rng.gen::<u16>() % min_len;
            let match_len = x.match_len(&y, start);

            assert!(
                match_len <= min_len,
                "{:?}.match_len({:?}, {}) = {}",
                x,
                y,
                start,
                match_len
            );

            for i in start..match_len {
                assert_eq!(
                    x.bit(i),
                    y.bit(i),
                    "{:?}.match_len({:?}, {}) = {}",
                    x,
                    y,
                    start,
                    match_len
                );
            }

            if match_len < min_len {
                assert_ne!(
                    x.bit(match_len),
                    y.bit(match_len),
                    "{:?}.match_len({:?}, {}) = {}",
                    x,
                    y,
                    start,
                    match_len
                );
            }
        }
    }
}

#[test]
fn test_proof_path_storage_key_leaf() {
    let key = ProofPath::new(&[250; 32]);
    let mut buf = vec![0; PROOF_PATH_SIZE];
    key.write(&mut buf);
    let key2 = ProofPath::read(&buf);

    assert_eq!(buf[0], LEAF_KEY_PREFIX);
    assert_eq!(buf[33], 0);
    assert_eq!(&buf[1..33], &[250; 32]);
    assert_eq!(key2, key);
}

#[test]
fn test_proof_path_storage_key_branch() {
    let mut key = ProofPath::new(&[255_u8; 32]);
    key = key.prefix(11);
    key = key.suffix(5);

    let mut buf = vec![0; PROOF_PATH_SIZE];
    key.write(&mut buf);
    let mut key2 = ProofPath::read(&buf);
    key2.start = 5;

    assert_eq!(buf[0], BRANCH_KEY_PREFIX);
    assert_eq!(buf[33], 11);
    assert_eq!(&buf[1..3], &[255, 7]);
    assert_eq!(&buf[3..33], &[0; 30]);
    assert_eq!(key2, key);
}

#[test]
fn test_proof_path_suffix() {
    let b = ProofPath::from_raw(*b"\x00\x01\x02\xFF\x0C0000000000000000000000000000\x20");

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
fn test_proof_path_prefix() {
    // spell-checker:disable
    let b = ProofPath::from_raw(*b"\x00\x83wertyuiopasdfghjklzxcvbnm123456\x08");
    assert_eq!(b.len(), 8);
    assert_eq!(b.prefix(1).bit(0), ChildKind::Right);
    assert_eq!(b.prefix(1).len(), 1);
}

#[test]
fn test_proof_path_len() {
    let b = ProofPath::from_raw(*b"\x01qwertyuiopasdfghjklzxcvbnm123456\x00");
    assert_eq!(b.len(), 256);
}

#[test]
#[should_panic(expected = "self.start() + idx < self.end()")]
fn test_proof_path_at_overflow() {
    let b = ProofPath::from_raw(*b"\x00qwertyuiopasdfghjklzxcvbnm123456\x0F");
    b.bit(32);
}

#[test]
#[should_panic(expected = "pos <= self.end()")]
fn test_proof_path_suffix_overflow() {
    let b = ProofPath::from_raw(*b"\x00qwertyuiopasdfghjklzxcvbnm123456\xFF");
    assert_eq!(b"\x01qwertyuiopasdfghjklzxcvbnm123456\x00".len(), 34);
    b.suffix(255).suffix(2);
}

#[test]
#[should_panic(expected = "self.start() + idx < self.end()")]
fn test_proof_path_suffix_bit_overflow() {
    let b = ProofPath::from_raw(*b"\x00qwertyuiopasdfghjklzxcvbnm123456\xFF");
    b.suffix(1).bit(255);
}

#[test]
fn test_proof_path_common_prefix_len() {
    let b1 = ProofPath::from_raw(*b"\x01abcd0000000000000000000000000000\x00");
    let b2 = ProofPath::from_raw(*b"\x01abef0000000000000000000000000000\x00");
    assert_eq!(b1.common_prefix_len(&b1), 256);
    let c = b1.common_prefix_len(&b2);
    assert_eq!(c, 17);
    let c = b2.common_prefix_len(&b1);
    assert_eq!(c, 17);
    let b1 = b1.suffix(9);
    let b2 = b2.suffix(9);
    let c = b1.common_prefix_len(&b2);
    assert_eq!(c, 8);
    let b3 = ProofPath::from_raw(*b"\x01\xFF0000000000000000000000000000000\x00");
    let b4 = ProofPath::from_raw(*b"\x01\xF70000000000000000000000000000000\x00");
    assert_eq!(b3.common_prefix_len(&b4), 3);
    assert_eq!(b4.common_prefix_len(&b3), 3);
    assert_eq!(b3.common_prefix_len(&b3), 256);
    let b3 = b3.suffix(30);
    assert_eq!(b3.common_prefix_len(&b3), 226);
    let b3 = b3.prefix(200);
    assert_eq!(b3.common_prefix_len(&b3), 200);
    let b5 = ProofPath::from_raw(*b"\x01\xF00000000000000000000000000000000\x00");
    assert_eq!(b5.prefix(0).common_prefix_len(&b3), 0);
}

#[test]
fn test_proof_path_match_len() {
    let b1 = ProofPath::from_raw(*b"\x01abcd0000000000000000000000000000\x00");
    let b2 = ProofPath::from_raw(*b"\x01abef0000000000000000000000000000\x00");

    for start in 0..256 {
        assert_eq!(b1.match_len(&b1, start), 256);
    }
    for start in 0..18 {
        assert_eq!(b1.match_len(&b2, start), 17);
        assert_eq!(b2.match_len(&b1, start), 17);
    }
    for start in 32..256 {
        assert_eq!(b1.match_len(&b2, start), 256);
        assert_eq!(b2.match_len(&b1, start), 256);
    }

    let b2 = ProofPath::from_raw(*b"\x01abce0000000000000000000000000000\x00");
    for start in 0..25 {
        assert_eq!(b1.match_len(&b2, start), 24);
        assert_eq!(b2.match_len(&b1, start), 24);
    }

    let b1 = b1.prefix(19);
    for start in 0..19 {
        assert_eq!(b1.match_len(&b2, start), 19);
        assert_eq!(b2.match_len(&b1, start), 19);
    }
}

#[test]
fn test_proof_path_is_leaf() {
    let b = ProofPath::from_raw(*b"\x01qwertyuiopasdfghjklzxcvbnm123456\x00");
    assert_eq!(b.len(), 256);
    assert_eq!(b.suffix(4).is_leaf(), true);
    assert_eq!(b.suffix(8).is_leaf(), true);
    assert_eq!(b.suffix(250).is_leaf(), true);
    assert_eq!(b.prefix(16).is_leaf(), false);
}

#[test]
fn test_proof_path_is_branch() {
    let b = ProofPath::from_raw(*b"\x00qwertyuiopasdfghjklzxcvbnm123456\xFF");
    assert_eq!(b.len(), 255);
    assert_eq!(b.is_leaf(), false);
}

#[test]
fn test_proof_path_debug_leaf() {
    use std::fmt::Write;
    let b = ProofPath::from_raw(*b"\x01qwertyuiopasdfghjklzxcvbnm123456\x00");
    let mut buf = String::new();
    write!(&mut buf, "{:?}", b).unwrap();
    assert_eq!(
        buf,
        "ProofPath { start: 0, end: 256, bits: \"01110001|01110111|01100101|01110010|01110100|0111\
         1001|01110101|01101001|01101111|01110000|01100001|01110011|01100100|01100110|01100111|0110\
         1000|01101010|01101011|01101100|01111010|01111000|01100011|01110110|01100010|01101110|0110\
         1101|00110001|00110010|00110011|00110100|00110101|00110110|\" }"
    );
}

#[test]
fn test_proof_path_debug_branch() {
    use std::fmt::Write;
    let b = ProofPath::from_raw(*b"\x00qwertyuiopasdfghjklzxcvbnm123456\xF0").suffix(12);
    let mut buf = String::new();
    write!(&mut buf, "{:?}", b).unwrap();
    assert_eq!(
        buf,
        "ProofPath { start: 12, end: 240, bits: \"________|0111____|01100101|01110010|01110100|011\
         11001|01110101|01101001|01101111|01110000|01100001|01110011|01100100|01100110|01100111|011\
         01000|01101010|01101011|01101100|01111010|01111000|01100011|01110110|01100010|01101110|011\
         01101|00110001|00110010|00110011|00110100|________|________|\" }"
    );
}
