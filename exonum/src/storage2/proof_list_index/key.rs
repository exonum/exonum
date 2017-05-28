const HEIGHT_SHIFT : u64 = 58;
// TODO: add checks for overflow
const MAX_LENGTH : u64 = 288230376151711743; // 2 ** 58 - 1

struct ListIndexKey {
    height: u64,
    index: u64
}

impl ListIndexKey {
    fn as_db_key(&self) -> u64 {
        debug_assert!(self.height <= 58 && self.index <= MAX_LENGTH);
        (self.height << HEIGHT_SHIFT) + self.index
    }

    fn from_db_key(key: u64) -> Self {
        Self {
            height: key >> HEIGHT_SHIFT,
            index: key & MAX_LENGTH
        }
    }

    fn parent(&self) -> Self {
        Self { height: height + 1, index: index >> 1 }
    }

    fn left(&self) -> Self {
        Self { height: height - 1, index << 1 }
    }

    fn right(&self) -> Self {
        Self { height: height - 1, index << 1 + 1 }
    }
}

impl StorageKey for ListIndexKey {
    fn size() -> usize {
        <u64 as StorageKey>::size()
    }

    fn write(&self, buffer: &mut Vec<u8>) {
        StorageKey::write(self.as_db_key(), buffer)
    }

    fn from_slice(buffer: &[u8]) -> Self {
        Self::from_db_key(StorageKey::from_slice(buffer))
    }
}
