use super::super::StorageKey;

const HEIGHT_SHIFT : u64 = 58;
// TODO: add checks for overflow
const MAX_LENGTH : u64 = 288230376151711743; // 2 ** 58 - 1

struct ListIndexKey {
    index: u64,
    height: u8
}

impl ListIndexKey {
    fn new(height: u8, index: u64) -> Self {
        debug_assert!(height <= 58 && index <= (1 << height));
        Self { height: height, index: index }
    }

    fn as_db_key(&self) -> u64 {
        ((self.height as u64) << HEIGHT_SHIFT) + self.index
    }

    fn from_db_key(key: u64) -> Self {
        Self::new((key >> HEIGHT_SHIFT) as u8, key & MAX_LENGTH)
    }

    fn parent(&self) -> Self {
        Self::new(self.height + 1, self.index >> 1)
    }

    fn left(&self) -> Self {
        Self::new(self.height - 1, self.index << 1)
    }

    fn right(&self) -> Self {
        Self::new(self.height - 1, self.index << 1 + 1)
    }
}

impl StorageKey for ListIndexKey {
    fn size() -> usize {
        <u64 as StorageKey>::size()
    }

    fn write(&self, buffer: &mut Vec<u8>) {
        StorageKey::write(&self.as_db_key(), buffer)
    }

    fn from_slice(buffer: &[u8]) -> Self {
        Self::from_db_key(StorageKey::from_slice(buffer))
    }
}
