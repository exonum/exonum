// Copyright 2020 The Exonum Team
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

use serde_derive::*;

use std::cmp::Ordering;

use crate::BinaryKey;

pub(crate) const HEIGHT_SHIFT: u64 = 56;
pub(crate) const MAX_INDEX: u64 = 0xFF_FFFF_FFFF_FFFF; // 2_u64.pow(56) - 1

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProofListKey {
    index: u64,
    height: u8,
}

impl ProofListKey {
    pub fn new(height: u8, index: u64) -> Self {
        debug_assert!(u64::from(height) <= HEIGHT_SHIFT && index <= MAX_INDEX);
        Self { height, index }
    }

    pub fn height(&self) -> u8 {
        self.height
    }

    pub fn index(&self) -> u64 {
        self.index
    }

    /// Checks if a key is valid. An invalid key may be obtained, for example, by deserializing
    /// untrusted input.
    pub fn is_valid(&self) -> bool {
        u64::from(self.height) <= HEIGHT_SHIFT && self.index <= MAX_INDEX
    }

    pub fn leaf(index: u64) -> Self {
        Self::new(0, index)
    }

    pub fn as_db_key(&self) -> u64 {
        (u64::from(self.height) << HEIGHT_SHIFT) + self.index
    }

    pub fn from_db_key(key: u64) -> Self {
        Self::new((key >> HEIGHT_SHIFT) as u8, key & MAX_INDEX)
    }

    pub fn parent(&self) -> Self {
        Self::new(self.height + 1, self.index >> 1)
    }

    pub fn first_left_leaf_index(&self) -> u64 {
        if self.height < 2 {
            self.index
        } else {
            self.index << (self.height - 1)
        }
    }

    pub fn is_left(&self) -> bool {
        self.index.trailing_zeros() >= 1
    }

    pub fn as_left(&self) -> Self {
        Self::new(self.height, self.index & !1)
    }

    pub fn as_right(&self) -> Self {
        Self::new(self.height, self.index | 1)
    }
}

impl BinaryKey for ProofListKey {
    fn size(&self) -> usize {
        8
    }

    fn write(&self, buffer: &mut [u8]) -> usize {
        BinaryKey::write(&self.as_db_key(), buffer);
        self.size()
    }

    fn read(buffer: &[u8]) -> Self {
        Self::from_db_key(<u64 as BinaryKey>::read(buffer))
    }
}

impl PartialOrd for ProofListKey {
    fn partial_cmp(&self, rhs: &Self) -> Option<Ordering> {
        Some(self.cmp(rhs))
    }
}

impl Ord for ProofListKey {
    fn cmp(&self, rhs: &Self) -> Ordering {
        let height_comparison = self.height.cmp(&rhs.height);
        if height_comparison == Ordering::Equal {
            self.index.cmp(&rhs.index)
        } else {
            height_comparison
        }
    }
}

#[test]
fn proof_list_key_ord() {
    assert!(ProofListKey::new(0, 1000) < ProofListKey::new(0, 1001));
    assert!(ProofListKey::new(0, 1000) < ProofListKey::new(1, 0));
    assert_eq!(ProofListKey::new(1, 100), ProofListKey::new(1, 100));
}
