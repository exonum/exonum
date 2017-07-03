use super::super::StorageKey;

const HEIGHT_SHIFT: u64 = 56;
const MAX_INDEX: u64 = 72057594037927935; // 2 ** 56 - 1

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
        ((self.height as u64) << HEIGHT_SHIFT) + self.index
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
        self.index & 1 == 0
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
