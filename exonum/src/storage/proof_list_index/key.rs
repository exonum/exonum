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

use super::super::StorageKey;

const HEIGHT_SHIFT: u64 = 56;
const MAX_INDEX: u64 = 0xFF_FFFF_FFFF_FFFF; // 2u64.pow(56) - 1

#[derive(Debug, Copy, Clone)]
pub struct ProofListKey {
    index: u64,
    height: u8,
}

impl ProofListKey {
    pub fn new(height: u8, index: u64) -> Self {
        debug_assert!(height <= 58 && index <= MAX_INDEX);
        Self {
            height: height,
            index: index,
        }
    }

    pub fn height(&self) -> u8 {
        self.height
    }

    pub fn index(&self) -> u64 {
        self.index
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

    pub fn left(&self) -> Self {
        Self::new(self.height - 1, self.index << 1)
    }

    pub fn right(&self) -> Self {
        Self::new(self.height - 1, (self.index << 1) + 1)
    }

    pub fn first_left_leaf_index(&self) -> u64 {
        if self.height < 2 {
            self.index
        } else {
            self.index << (self.height - 1)
        }
    }

    pub fn first_right_leaf_index(&self) -> u64 {
        if self.height < 2 {
            self.index
        } else {
            ((self.index << 1) + 1) << (self.height - 2)
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

impl StorageKey for ProofListKey {
    fn size(&self) -> usize {
        8
    }

    fn write(&self, buffer: &mut [u8]) {
        StorageKey::write(&self.as_db_key(), buffer)
    }

    fn read(buffer: &[u8]) -> Self {
        Self::from_db_key(StorageKey::read(buffer))
    }
}
