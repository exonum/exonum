use std::collections::BTreeMap;

use time::Timespec;
use byteorder::{ByteOrder, LittleEndian};

use super::messages::{Message, Propose, Precommit, TxMessage};
use super::crypto::{Hash, hash};

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
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

    fn block_hash(&self) -> Hash;

    fn height(&self) -> Height;

    fn prev_hash(&self) -> Hash {
        self.get_block(self.height().height()).unwrap()
    }

    fn prev_time(&self) -> Timespec {
        // TODO: Possibly inefficient
        self.get_propose(&self.prev_hash()).unwrap().time()
    }

    fn get_tx(&self, hash: &Hash) -> Option<TxMessage>;
    fn get_propose(&self, hash: &Hash) -> Option<Propose>;
    fn get_precommits(&self, hash: &Hash) -> Option<Vec<Precommit>>;

    fn get_block(&self, height: u64) -> Option<Hash>;
    // fn put_block(&mut self, height: Height, propose: Propose);

    fn merge(&mut self, patch: &Patch);
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
    blocks: BTreeMap<Height, Hash>,
    proposes: BTreeMap<Hash, Propose>,
    precommits: BTreeMap<Hash, Vec<Precommit>>,
    txs: BTreeMap<Hash, TxMessage>,
}

impl MemoryStorage {
    pub fn new() -> MemoryStorage {
        MemoryStorage {
            blocks: BTreeMap::new(),
            proposes: BTreeMap::new(),
            precommits: BTreeMap::new(),
            txs: BTreeMap::new()
        }
    }
}

impl Storage for MemoryStorage {

    fn block_hash(&self) -> Hash {
        self.prev_hash()
    }

    fn height(&self) -> Height {
        *self.blocks.keys().last().unwrap()
    }

    fn get_block(&self, height: u64) -> Option<Hash> {
        self.blocks.get(&Height::new(height)).map(|b| b.clone())
    }

    fn get_propose(&self, hash: &Hash) -> Option<Propose> {
        self.proposes.get(hash).map(|x| x.clone())
    }

    fn get_tx(&self, hash: &Hash) -> Option<TxMessage> {
        self.txs.get(hash).map(|x| x.clone())
    }

    fn get_precommits(&self, hash: &Hash) -> Option<Vec<Precommit>> {
        self.precommits.get(hash).map(|x| x.clone())
    }

    // fn put_block(&mut self, height: Height, block: Propose) {
    //     self.blocks.insert(height, block);
    // }

    fn merge(&mut self, patch: &Patch) {
        // for change in &patch.changes {
        //     match *change {
        //         Change::PutBlock(ref height, ref block)
        //             => self.put_block(*height, block.clone()),
        //     }
        // }
    }
}

pub struct Fork<'a, S: Storage + 'a + ?Sized> {
    storage: &'a S,
    changes: MemoryStorage
}

impl<'a, S: Storage + 'a + ?Sized> Fork<'a, S> {
    pub fn new(storage: &'a S) -> Fork<'a, S> {
        Fork {
            storage: storage,
            changes: MemoryStorage::new(),
        }
    }

    pub fn patch(self) -> Patch {
        let block_hash = self.block_hash().clone();
        let mut changes = Vec::new();

        // changes.extend(self.changes.blocks
        //                    .into_iter().map(|(k, v)| Change::PutBlock(k, v)));

        Patch {
            block_hash: block_hash,
            changes: changes
        }
    }
}

impl<'a, S: Storage + 'a + ?Sized> Storage for Fork<'a, S> {
    fn block_hash(&self) -> Hash {
        self.prev_hash()
    }

    fn height(&self) -> Height {
        ::std::cmp::max(self.changes.height(), self.storage.height())
    }

    fn get_block(&self, height: u64) -> Option<Hash> {
        self.changes.get_block(height)
                    .or_else(|| self.storage.get_block(height))
    }

    fn get_propose(&self, hash: &Hash) -> Option<Propose> {
        self.changes.get_propose(hash)
                    .or_else(|| self.storage.get_propose(hash))
    }

    fn get_precommits(&self, hash: &Hash) -> Option<Vec<Precommit>> {
        self.changes.get_precommits(hash)
                    .or_else(|| self.storage.get_precommits(hash))
    }

    fn get_tx(&self, hash: &Hash) -> Option<TxMessage> {
        self.changes.get_tx(hash)
                    .or_else(|| self.storage.get_tx(hash))
    }

    fn merge(&mut self, patch: &Patch) {
        self.changes.merge(patch);
    }
}

pub struct Patch {
    block_hash: Hash,
    changes: Vec<Change>
}

pub enum Change {
    PutBlock(Height, Propose)
}

impl Patch {

    pub fn block_hash(&self) -> &Hash {
        &self.block_hash
    }
}
