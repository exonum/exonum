struct ListIndexKey {
    height: u64,
    index: u64
}

impl ListIndexKey {
    fn as_db_key(&self) -> u64 {

    }

    fn from_db_key(key: u64) -> Self {

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
