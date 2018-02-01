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

pub const BRANCH_KEY_PREFIX: u8 = 0;
pub const LEAF_KEY_PREFIX: u8 = 1;

/// Size in bytes of the `ProofMapKey`.
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

/// A structure that represents paths to `ProofMapIndex` nodes.
///
/// # Binary representation
///
/// | Position in bytes     | Description                   	                    |
/// |-------------------    |----------------------------------------------         |
/// | 0               	    | `ProofPath` kind: (0 is branch, 1 is leaf)            |
/// | 1..33                 | `ProofMapKey` bytes.    	                            |
/// | 33                    | Total length in bits of `ProofMapKey` for branches.   |
///
#[derive(Copy, Clone)]
pub struct ProofPath {
    bytes: [u8; PROOF_PATH_SIZE],
    start: u16,
}

impl ProofPath {
    /// Create a path from the given key.
    pub fn new<K: ProofMapKey>(key: &K) -> ProofPath {
        debug_assert_eq!(key.size(), KEY_SIZE);

        let mut data = [0; PROOF_PATH_SIZE];
        data[0] = LEAF_KEY_PREFIX;
        key.write(&mut data[1..KEY_SIZE + 1]);
        data[PROOF_PATH_LEN_POS] = 0;
        ProofPath::from_raw(data)
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
    fn from_raw(raw: [u8; PROOF_PATH_SIZE]) -> ProofPath {
        debug_assert!(
            (raw[PROOF_PATH_KIND_POS] != LEAF_KEY_PREFIX) || (raw[PROOF_PATH_LEN_POS] == 0),
            "ProofPath is inconsistent"
        );

        ProofPath {
            bytes: raw,
            start: 0,
        }
    }

    /// Sets the right border of the bit range.
    fn set_end(&mut self, end: Option<u8>) {
        // Update ProofPath kind and right bound.
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

    /// Get bit at index `idx`.
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

    /// Returns the copy of this bit range with the given left border.
    fn start_from(&self, pos: u16) -> Self;

    /// Returns a copy of this bit range shortened to the specified length.
    fn prefix(&self, len: u16) -> Self;

    /// Returns the copy of this bit range where the start is shifted by the `len`
    /// bits to the right.
    fn suffix(&self, len: u16) -> Self;

    /// Checks if this bit range contains the other bit range as a prefix,
    /// provided that the start positions of both ranges are the same.
    fn starts_with(&self, other: &Self) -> bool {
        self.common_prefix_len(other) == other.len()
    }

    /// Returns the raw bytes of the key.
    fn raw_key(&self) -> &[u8];

    /// Returns the length of the common prefix between this and the other range,
    /// provided that they start from the same position.
    /// If start positions differ, returns 0.
    fn common_prefix_len(&self, other: &Self) -> u16 {
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

        let mut key = ProofPath::from_raw(self.bytes);
        key.start = pos;
        key
    }

    fn prefix(&self, len: u16) -> Self {
        let end = self.start + len;
        let key_len = self.raw_key().len() as u16 * 8;
        debug_assert!(end < key_len);

        let mut key = ProofPath::from_raw(self.bytes);
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
                buffer[right] &= !(255u8 << (self.end() % 8));
            }
            for i in buffer.iter_mut().take(KEY_SIZE + 1).skip(right + 1) {
                *i = 0
            }
        }
    }

    fn read(buffer: &[u8]) -> Self {
        debug_assert_eq!(buffer.len(), PROOF_PATH_SIZE);
        let mut data = [0; PROOF_PATH_SIZE];
        data.copy_from_slice(buffer);
        ProofPath::from_raw(data)
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
    let mut key = ProofPath::new(&[255u8; 32]);
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
