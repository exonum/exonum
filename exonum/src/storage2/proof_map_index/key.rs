use std::cmp::min;

use crypto::{Hash, PublicKey, HASH_SIZE};

use super::super::StorageKey;

pub const BRANCH_KEY_PREFIX: u8 = 00;
pub const LEAF_KEY_PREFIX: u8 = 01;

pub const KEY_SIZE: usize = HASH_SIZE;
pub const DB_KEY_SIZE: usize = KEY_SIZE + 2;

pub trait ProofMapKey : StorageKey {}

impl ProofMapKey for Hash {}
impl ProofMapKey for PublicKey {}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ChildKind {
    Left,
    Right,
}

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
        debug_assert!(K::size() == KEY_SIZE);

        let mut data = [0; KEY_SIZE];
        key.write(&mut data);
        DBKey { data: data, from: 0, to: KEY_SIZE as u16 }
    }

    pub fn from(&self) -> u16 {
        self.from
    }

    pub fn to(&self) -> u16 {
        self.to
    }

    /// Length of the `DBKey`
    pub fn len(&self) -> u16 {
        self.to - self.from
    }

    /// Returns true if `DBKey` has zero length
    pub fn is_empty(&self) -> bool {
        self.to == self.from
    }

    /// Get bit at position `idx`.
    pub fn get(&self, idx: u16) -> ChildKind {
        debug_assert!(!self.is_empty() && idx < self.to);

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
        DBKey { data: self.data, from: self.from, to: self.from + mid }
    }

    /// Return object which represents a view on to this slice (further) offset by `i` bits.
    pub fn suffix(&self, mid: u16) -> DBKey {
        debug_assert!(self.from + mid <= self.to);

        DBKey { data: self.data, from: self.from + mid, to: self.to }
    }

    /// Returns how many bits at the beginning matches with `other`
    pub fn common_prefix(&self, other: &Self) -> u16 {
        // We assume that all slices created from byte arrays with the same length
        if self.from != other.from {
            0
        } else {
            let from = self.from / 8;
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
    }

    /// Returns true if we starts with the same prefix at the whole of `Other`
    pub fn starts_with(&self, other: &Self) -> bool {
        self.common_prefix(other) == other.len()
    }

    /// Returns true if self.to not changed
    pub fn is_leaf(&self) -> bool {
        debug_assert!(self.from == 0);

        self.to == KEY_SIZE as u16
    }
}

impl AsRef<[u8]> for DBKey {
    fn as_ref(&self) -> &[u8] {
        &self.data
    }
}

impl StorageKey for DBKey {
    fn size() -> usize {
        DB_KEY_SIZE
    }

    fn write(&self, buffer: &mut [u8]) {
        if self.is_leaf() {
            buffer[0] = LEAF_KEY_PREFIX;
            buffer[1..KEY_SIZE + 1].copy_from_slice(&self.data);
            buffer[KEY_SIZE + 1] = 0;
        } else {
            buffer[0] = BRANCH_KEY_PREFIX;
            buffer[1..KEY_SIZE + 1].copy_from_slice(&self.data);
            buffer[KEY_SIZE + 1] = self.to as u8;
        }
    }

    fn read(buffer: &[u8]) -> Self {
        let mut data = [0; KEY_SIZE];
        data[..].copy_from_slice(&buffer[1..KEY_SIZE + 1]);
        let to = match buffer[0] {
            LEAF_KEY_PREFIX => KEY_SIZE as u16 * 8,
            BRANCH_KEY_PREFIX => buffer[DB_KEY_SIZE - 1] as u16,
            _ => unreachable!("wrong key prefix")
        };
        DBKey { data: data, from: 0, to: to }
    }
}

impl PartialEq for DBKey {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len() && self.starts_with(other)
    }
}

impl ::std::fmt::Debug for DBKey {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        write!(f, "DBKey(")?;
        for i in self.to..self.from {
            write!(f, "{}", match self.get(i) {
                ChildKind::Left => '0',
                ChildKind::Right => '1'
            })?;
        }
        write!(f, ")")
    }
}
