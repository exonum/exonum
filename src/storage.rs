use std::collections::BTreeMap;

use byteorder::{ByteOrder, LittleEndian};

use super::messages::Propose;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Height([u8; 8]);

impl Height {
    pub fn new(height: u64) -> Height {
        let mut buf = [0, 0, 0, 0, 0, 0, 0, 0];
        LittleEndian::write_u64(&mut buf, height);
        Height(buf)
    }

    pub fn height(&self) -> u64 {
        match *self {
            Height(ref height) => LittleEndian::read_u64(height)
        }
    }
}

impl AsRef<[u8]> for Height {
    fn as_ref(&self) -> &[u8] {
        match *self {
            Height(ref buf) => buf
        }
    }
}

pub trait Storage {
    // type Blocks: Table<Height, Propose>;

    // fn blocks(&self) -> &Self::Blocks;
    // fn blocks_mut(&mut self) -> &mut Self::Blocks;

    fn height(&self) -> Height;

    fn prev_hash(&self) -> Hash {
        // TODO: Possibly inefficient
        self.get_block(self.height()).hash()
    }

    fn get_block(&self, height: &Height) -> Option<Propose>;
    fn put_block(&mut self, height: Height, propose: Propose);

    fn merge(&mut self, changes: Changes);
}

// trait Table<K, V> where K: AsRef<[u8]> {
//     fn get(&self, k: &K) -> Option<&V>;
//     fn put(&mut self, k: K, v: V);
// }

// impl<K, V> Table<K, V> for BTreeMap<Vec<u8>, V> where K: AsRef<[u8]> {
//     fn get(&self, k: &K) -> Option<&V> {
//         self.get(k.as_ref())
//     }

//     fn put(&mut self, k: K, v: V) {
//         self.insert(k.as_ref().to_vec(), v);
//     }
// }

pub struct MemoryStorage {
    blocks: BTreeMap<Height, Propose>
}

impl MemoryStorage {
    pub fn new() -> MemoryStorage {
        MemoryStorage {
            blocks: BTreeMap::new()
        }
    }
}

impl Storage for MemoryStorage {
    fn get_block(&self, height: &Height) -> Option<Propose> {
        self.blocks.get(height).map(|b| b.clone())
    }

    fn put_block(&mut self, height: Height, block: Propose) {
        self.blocks.insert(height, block);
    }

    fn merge(&mut self, changes: Changes) {
        for change in changes {
            match change {
                Change::PutBlock(height, block)
                    => self.put_block(height, block),
            }
        }
    }
}

struct Fork<'a, S: Storage + 'a> {
    storage: &'a S,
    changes: MemoryStorage
}

impl<'a, S: Storage + 'a> Fork<'a, S> {
    pub fn new(storage: &'a S) -> Fork<'a, S> {
        Fork {
            storage: storage,
            changes: MemoryStorage::new(),
        }
    }

    pub fn changes(self) -> Changes {
        let mut changes = Changes::new();

        changes.extend(self.changes.blocks
                       .into_iter().map(|(k, v)| Change::PutBlock(k, v)));

        changes
    }
}

impl<'a, S: Storage + 'a> Storage for Fork<'a, S> {
    fn get_block(&self, height: &Height) -> Option<Propose> {
        self.changes.get_block(height)
                    .or_else(|| self.storage.get_block(height))
    }

    fn put_block(&mut self, height: Height, block: Propose) {
        self.changes.put_block(height, block);
    }

    fn merge(&mut self, changes: Changes) {
        self.changes.merge(changes);
    }
}

pub type Changes = Vec<Change>;

pub enum Change {
    PutBlock(Height, Propose)
}
