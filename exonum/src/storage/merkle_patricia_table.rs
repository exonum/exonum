use std::mem;
use std::cmp::{min, PartialEq};
use std::marker::PhantomData;
use std::fmt;
use std::ops::Not;

use ::crypto::{hash, Hash, HASH_SIZE};

use super::{Map, Error, StorageValue};

const BRANCH_KEY_PREFIX: u8 = 00;
const LEAF_KEY_PREFIX: u8 = 01;

const KEY_SIZE: usize = HASH_SIZE;
const DB_KEY_SIZE: usize = KEY_SIZE + 2;
const BRANCH_NODE_SIZE: usize = 2 * (HASH_SIZE + DB_KEY_SIZE);

#[derive(Debug, Clone, Copy, PartialEq)]
enum ChildKind {
    Left,
    Right,
}

struct BitSlice<'a> {
    data: &'a [u8],
    from: u16,
    to: u16,
}

impl Not for ChildKind {
    type Output = ChildKind;

    fn not(self) -> ChildKind {
        match self {
            ChildKind::Left => ChildKind::Right,
            ChildKind::Right => ChildKind::Left,
        }
    }
}

// TODO combine bitslice with db prefix and add cache to avoid reallocations
impl<'a> BitSlice<'a> {
    /// Create a new bit slice from the given binary data.
    fn from_bytes(bytes: &'a [u8]) -> BitSlice {
        debug_assert!(bytes.len() <= KEY_SIZE);

        let to = if bytes.is_empty() {
            0
        } else {
            (bytes.len() * 8) as u16
        };
        BitSlice {
            data: bytes,
            from: 0,
            to: to,
        }
    }
    /// Length of the `BitSlice`
    fn len(&self) -> usize {
        (self.to - self.from) as usize
    }
    /// Returns true if `BitSlice` has zero length
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Get bit at position `idx`.
    fn at(&self, idx: usize) -> ChildKind {
        debug_assert!(!self.is_empty());
        debug_assert!((idx as u16) < self.to);

        let pos = self.from + idx as u16;
        let chunk = self.data[(pos / 8) as usize];
        let bit = 7 - pos % 8;
        let value = (1 << bit) & chunk;
        if value != 0 {
            ChildKind::Right
        } else {
            ChildKind::Left
        }
    }
    /// Return object which represents a view on to this slice (further) offset by `i` bits.
    fn mid(&self, sz: usize) -> BitSlice {
        debug_assert!(self.from + sz as u16 <= self.to);

        BitSlice {
            data: self.data,
            from: self.from + sz as u16,
            to: self.to,
        }
    }
    /// Shortens this BitSlice to the specified length.
    fn truncate(&self, sz: usize) -> BitSlice {
        BitSlice {
            data: self.data,
            from: self.from,
            to: self.from + sz as u16,
        }
    }
    /// Returns how many bits at the beginning matches with `other`
    fn common_prefix(&self, other: &Self) -> usize {
        // We assume that all slices created from byte arrays with the same length
        if self.from != other.from {
            0
        } else {
            let from = (self.from / 8) as usize;
            let to = min((self.to + 7) / 8, (other.to + 7) / 8) as usize;
            let max_len = min(self.len(), other.len());

            for i in from..to {
                let x = self.data[i] ^ other.data[i];
                if x != 0 {
                    let tail = x.leading_zeros() as usize;
                    return min(i * 8 + tail - self.from as usize, max_len);
                }
            }
            max_len
        }
    }
    /// Returns true if we starts with the same prefix at the whole of `Other`
    fn starts_with(&self, other: &Self) -> bool {
        self.common_prefix(other) == other.len()
    }
    /// Returns true if self.to not changed
    fn is_leaf_key(&self) -> bool {
        self.to == self.data.len() as u16 * 8
    }
    /// Converts slice to node key and mask tail instead cut.
    /// Also it writes len in bits on tail and adds prefix on head
    // ( 00 for branches and 01 for leaves )
    fn to_db_key(&self) -> Vec<u8> {
        if self.is_leaf_key() {
            let mut v = vec![LEAF_KEY_PREFIX];
            v.extend_from_slice(self.data);
            v.resize(DB_KEY_SIZE, 0u8);
            v
        } else {
            let right = (self.to as usize + 7) / 8;
            let mut v = Vec::new();
            v.resize(DB_KEY_SIZE, 0u8);
            v[0] = BRANCH_KEY_PREFIX;
            v[1..right + 1].copy_from_slice(&self.data[0..right]);
            if self.to % 8 != 0 {
                v[right] &= !(255u8 >> (self.to % 8));
            }
            v[DB_KEY_SIZE - 1] = self.to as u8;
            v
        }
    }
    fn from_db_key(key: &'a [u8]) -> BitSlice {
        match key[0] {
            LEAF_KEY_PREFIX => {
                BitSlice {
                    data: &key[1..KEY_SIZE + 1],
                    from: 0,
                    to: KEY_SIZE as u16 * 8,
                }
            }
            BRANCH_KEY_PREFIX => {
                BitSlice {
                    data: &key[1..key.len() - 1],
                    from: 0,
                    to: key[DB_KEY_SIZE - 1] as u16,
                }
            }
            _ => unreachable!("Wrong key prefix"),
        }
    }
}

impl<'a> PartialEq for BitSlice<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len() && self.starts_with(other)
    }
}

// TODO find more padding friendly data layout
// struct BranchData {
//     left_hash:      &Hash,       HASH_SIZE,
//     right_hash:     &Hash,       HASH_SIZE,
//     left_prefix:    &BitSlice    DB_KEY_SIZE,
//     right_prefix:   &BitSlice    DB_KEY_SIZE
// }
struct BranchNode {
    raw: Vec<u8>,
}

enum Node<T: StorageValue> {
    Leaf(T),
    Branch(BranchNode),
}

impl BranchNode {
    fn from_bytes(raw: Vec<u8>) -> BranchNode {
        debug_assert_eq!(raw.len(), BRANCH_NODE_SIZE);
        BranchNode { raw: raw }
    }
    fn empty() -> BranchNode {
        BranchNode { raw: vec![0; BRANCH_NODE_SIZE] }
    }

    fn hash(&self) -> Hash {
        hash(&[self.child_hash(ChildKind::Left).as_ref(),
               self.child_hash(ChildKind::Right).as_ref()]
            .concat())
    }

    fn child_hash(&self, kind: ChildKind) -> &Hash {
        // TODO move offset to trait or const or enum or macro?
        let from = match kind {
            ChildKind::Right => HASH_SIZE,
            ChildKind::Left => 0,
        };
        unsafe { self.read_hash(from) }
    }
    fn child_slice(&self, kind: ChildKind) -> BitSlice {
        let from = match kind {
            ChildKind::Right => 2 * HASH_SIZE + DB_KEY_SIZE,
            ChildKind::Left => 2 * HASH_SIZE,
        };
        self.read_slice(from)
    }
    fn set_child_slice(&mut self, kind: ChildKind, prefix: &BitSlice) {
        let from = match kind {
            ChildKind::Right => 2 * HASH_SIZE + DB_KEY_SIZE,
            ChildKind::Left => 2 * HASH_SIZE,
        };
        self.write_slice(from, prefix);
    }
    fn set_child_hash(&mut self, kind: ChildKind, hash: &Hash) {
        let from = match kind {
            ChildKind::Right => HASH_SIZE,
            ChildKind::Left => 0,
        };
        unsafe {
            self.write_hash(from, hash);
        }
    }
    fn set_child(&mut self, kind: ChildKind, prefix: &BitSlice, hash: &Hash) {
        self.set_child_slice(kind, prefix);
        self.set_child_hash(kind, hash);
    }

    // Think about truncate keys
    // fn child_db_key(&self, kind: ChildKind) -> &[u8] {
    //     let from = match kind {
    //         ChildKind::Right => 2 * HASH_SIZE + DB_KEY_SIZE,
    //         ChildKind::Left => 2 * HASH_SIZE,
    //     };
    //     self.read_db_key(from)
    // }
    // fn read_db_key(&self, from: usize) -> &[u8] {
    //     &self.raw[from..from+DB_KEY_SIZE]
    // }

    unsafe fn read_hash(&self, from: usize) -> &Hash {
        mem::transmute(&self.raw[from])
    }
    unsafe fn write_hash(&mut self, from: usize, hash: &Hash) -> usize {
        self.raw[from..from + HASH_SIZE].copy_from_slice(hash.as_ref());
        from + HASH_SIZE
    }
    fn read_slice(&self, from: usize) -> BitSlice {
        let to = match self.raw[from] {
            LEAF_KEY_PREFIX => (KEY_SIZE * 8) as u16,
            BRANCH_KEY_PREFIX => self.raw[from + DB_KEY_SIZE - 1] as u16,
            _ => unreachable!("Wrong key prefix"),
        };
        BitSlice {
            from: 0,
            to: to,
            data: &self.raw[from + 1..from + DB_KEY_SIZE - 1],
        }
    }
    fn write_slice(&mut self, from: usize, slice: &BitSlice) -> usize {
        debug_assert!(slice.data.len() <= KEY_SIZE);

        if slice.is_leaf_key() {
            self.raw[from] = LEAF_KEY_PREFIX;
            self.raw[from + DB_KEY_SIZE - 1] = 0;
        } else {
            self.raw[from] = BRANCH_KEY_PREFIX;
            self.raw[from + DB_KEY_SIZE - 1] = slice.to as u8;
        };
        self.raw[from + 1..from + DB_KEY_SIZE - 1].copy_from_slice(slice.data);
        from + DB_KEY_SIZE
    }
}

impl StorageValue for BranchNode {
    fn serialize(self) -> Vec<u8> {
        self.raw
    }

    fn deserialize(v: Vec<u8>) -> Self {
        BranchNode::from_bytes(v)
    }

    fn hash(&self) -> Hash {
        self.hash()
    }
}

pub struct MerklePatriciaTable<T: Map<[u8], Vec<u8>>, K: ?Sized, V> {
    map: T,
    _k: PhantomData<K>,
    _v: PhantomData<V>,
}

enum RemoveResult {
    KeyNotFound,
    Leaf,
    Branch((Vec<u8>, Hash)),
    UpdateHash(Hash),
}

type Entry<V> = (Vec<u8>, Node<V>);

// TODO avoid reallocations where is possible.
impl<'a, T: Map<[u8], Vec<u8>> + 'a, K: ?Sized, V: StorageValue> MerklePatriciaTable<T, K, V> {
    pub fn new(map: T) -> Self {
        MerklePatriciaTable {
            map: map,
            _k: PhantomData,
            _v: PhantomData,
        }
    }

    pub fn root_hash(&self) -> Result<Hash, Error> {
        match self.root_node()? {
            Some((prefix, Node::Leaf(value))) => {
                let prefix_slice = BitSlice::from_db_key(&prefix);
                Ok(Self::hash_leaf(&prefix_slice, &value))
            }
            Some((_, Node::Branch(branch))) => Ok(branch.hash()),
            None => Ok(Hash([0; 32])),
        }
    }

    fn root_node(&self) -> Result<Option<Entry<V>>, Error> {
        let out = match self.root_prefix()? {
            Some(prefix) => {
                let node = self.read_node(&prefix)?;
                Some((prefix, node))
            }
            None => None,
        };
        Ok(out)
    }

    fn insert<A: AsRef<[u8]>>(&self, key: A, value: V) -> Result<(), Error> {
        let key = key.as_ref();
        debug_assert_eq!(key.len(), KEY_SIZE);

        let key_slice = BitSlice::from_bytes(key);
        match self.root_node()? {
            Some((prefix, Node::Leaf(prefix_data))) => {
                let prefix_slice = BitSlice::from_db_key(&prefix);
                let i = prefix_slice.common_prefix(&key_slice);

                let leaf_hash = self.insert_leaf(&key_slice, value)?;
                if i < key_slice.len() {
                    let mut branch = BranchNode::empty();
                    branch.set_child(key_slice.at(i), &key_slice.mid(i), &leaf_hash);
                    branch.set_child(prefix_slice.at(i),
                                     &prefix_slice.mid(i),
                                     &Self::hash_leaf(&prefix_slice, &prefix_data));
                    let new_prefix = key_slice.truncate(i);
                    self.insert_branch(&new_prefix, branch)?;
                }
                Ok(())
            }
            Some((prefix, Node::Branch(mut branch))) => {
                let prefix_slice = BitSlice::from_db_key(&prefix);
                let i = prefix_slice.common_prefix(&key_slice);

                if i == prefix_slice.len() {
                    let suffix_slice = key_slice.mid(i);
                    // Just cut the prefix and recursively descent on.
                    let (j, h) = self.do_insert_branch(&branch, &suffix_slice, value)?;
                    match j {
                        Some(j) => {
                            branch.set_child(suffix_slice.at(0), &suffix_slice.truncate(j), &h)
                        }
                        None => branch.set_child_hash(suffix_slice.at(0), &h),
                    };
                    self.insert_branch(&prefix_slice, branch)?;
                } else {
                    // Inserts a new branch and adds current branch as its child
                    let hash = self.insert_leaf(&key_slice, value)?;
                    let mut new_branch = BranchNode::empty();
                    new_branch.set_child(prefix_slice.at(i), &prefix_slice.mid(i), &branch.hash());
                    new_branch.set_child(key_slice.at(i), &key_slice.mid(i), &hash);
                    // Saves a new branch
                    let new_prefix = prefix_slice.truncate(i);
                    self.insert_branch(&new_prefix, new_branch)?;
                }
                Ok(())
            }
            None => {
                // Eats hash
                self.insert_leaf(&key_slice, value).map(|_| ())
            }
        }
    }

    // Inserts a new node as child of current branch and returns updated hash
    // or if a new node has more short key returns a new key length
    fn do_insert_branch(&self,
                        parent: &BranchNode,
                        key_slice: &BitSlice,
                        value: V)
                        -> Result<(Option<usize>, Hash), Error> {
        let mut child_slice = parent.child_slice(key_slice.at(0));
        child_slice.from = key_slice.from;
        // If the slice is fully fit in key then there is a two cases
        let i = child_slice.common_prefix(key_slice);
        if child_slice.len() == i {
            // check that child is leaf to avoid unnecessary read
            if child_slice.is_leaf_key() {
                // there is a leaf in branch and we needs to update its value
                let hash = self.insert_leaf(key_slice, value)?;
                Ok((None, hash))
            } else {
                match self.read_node(child_slice.to_db_key())? {
                    Node::Leaf(_) => {
                        unreachable!("Something went wrong!");
                    }
                    // There is a child in branch and we needs to lookup it recursively
                    Node::Branch(mut branch) => {
                        let (j, h) = self.do_insert_branch(&branch, &key_slice.mid(i), value)?;
                        match j {
                            Some(j) => {
                                branch.set_child(key_slice.at(i), &key_slice.mid(i).truncate(j), &h)
                            }
                            None => branch.set_child_hash(key_slice.at(i), &h),
                        };
                        let hash = branch.hash();
                        self.insert_branch(&child_slice, branch)?;
                        Ok((None, hash))
                    }
                }
            }
        } else {
            // A simple case of inserting a new branch
            let suffix_slice = key_slice.mid(i);
            let mut new_branch = BranchNode::empty();
            // Add a new leaf
            let hash = self.insert_leaf(&suffix_slice, value)?;
            new_branch.set_child(suffix_slice.at(0), &suffix_slice, &hash);
            // Move current branch
            new_branch.set_child(child_slice.at(i),
                                 &child_slice.mid(i),
                                 parent.child_hash(key_slice.at(0)));

            let hash = new_branch.hash();
            self.insert_branch(&key_slice.truncate(i), new_branch)?;
            Ok((Some(i), hash))
        }
    }

    fn hash_leaf(key: &BitSlice, value: &V) -> Hash {
        hash(&[key.data, value.hash().as_ref()].concat())
    }

    fn remove(&self, key_slice: BitSlice) -> Result<(), Error> {
        match self.root_node()? {
            // If we have only on leaf, then we just need to remove it (if any)
            Some((prefix, Node::Leaf(_))) => {
                let key = key_slice.to_db_key();
                if key == prefix {
                    self.map.delete(&key)?;
                }
                Ok(())
            }
            Some((prefix, Node::Branch(mut branch))) => {
                // Truncate prefix
                let prefix_slice = BitSlice::from_db_key(&prefix);
                let i = prefix_slice.common_prefix(&key_slice);
                if i == prefix_slice.len() {
                    let suffix_slice = key_slice.mid(i);
                    match self.do_remove_node(&branch, &suffix_slice)? {
                        RemoveResult::Leaf => {
                            self.map.delete(&prefix)?;
                        }
                        RemoveResult::Branch((key, hash)) => {
                            let mut new_child_slice = BitSlice::from_db_key(key.as_ref());
                            new_child_slice.from = suffix_slice.from;
                            branch.set_child(suffix_slice.at(0), &new_child_slice, &hash);
                            self.insert_branch(&prefix_slice, branch)?;
                        }
                        RemoveResult::UpdateHash(hash) => {
                            branch.set_child_hash(suffix_slice.at(0), &hash);
                            self.insert_branch(&prefix_slice, branch)?;
                        }
                        RemoveResult::KeyNotFound => {
                            return Ok(());
                        }
                    }
                }
                Ok(())
            }
            None => Ok(()),
        }
    }

    fn do_remove_node(&self,
                      parent: &BranchNode,
                      key_slice: &BitSlice)
                      -> Result<RemoveResult, Error> {
        let mut child_slice = parent.child_slice(key_slice.at(0));
        child_slice.from = key_slice.from;
        let i = child_slice.common_prefix(key_slice);

        if i == child_slice.len() {
            match self.read_node(child_slice.to_db_key())? {
                Node::Leaf(_) => {
                    self.map.delete(&key_slice.to_db_key())?;
                    return Ok(RemoveResult::Leaf);
                }
                Node::Branch(mut branch) => {
                    let suffix_slice = key_slice.mid(i);
                    match self.do_remove_node(&branch, &suffix_slice)? {
                        RemoveResult::Leaf => {
                            let child = !suffix_slice.at(0);
                            let key = branch.child_slice(child).to_db_key();
                            let hash = branch.child_hash(child);

                            self.map.delete(&child_slice.to_db_key())?;

                            return Ok(RemoveResult::Branch((key, *hash)));
                        }
                        RemoveResult::Branch((key, hash)) => {
                            let mut new_child_slice = BitSlice::from_db_key(key.as_ref());
                            new_child_slice.from = suffix_slice.from;

                            branch.set_child(suffix_slice.at(0), &new_child_slice, &hash);
                            let h = branch.hash();
                            self.insert_branch(&child_slice, branch)?;
                            return Ok(RemoveResult::UpdateHash(h));
                        }
                        RemoveResult::UpdateHash(hash) => {
                            branch.set_child_hash(suffix_slice.at(0), &hash);
                            let h = branch.hash();
                            self.insert_branch(&child_slice, branch)?;
                            return Ok(RemoveResult::UpdateHash(h));
                        }
                        RemoveResult::KeyNotFound => {
                            return Ok(RemoveResult::KeyNotFound);
                        }
                    }
                }
            }
        }
        Ok(RemoveResult::KeyNotFound)
    }

    fn root_prefix(&self) -> Result<Option<Vec<u8>>, Error> {
        self.map.find_key(&[])
    }

    fn read_node<A: AsRef<[u8]>>(&self, key: A) -> Result<Node<V>, Error> {
        let db_key = key.as_ref();
        match self.map.get(db_key.as_ref())? {
            Some(data) => {
                match db_key[0] {
                    LEAF_KEY_PREFIX => Ok(Node::Leaf(StorageValue::deserialize(data))),
                    BRANCH_KEY_PREFIX => Ok(Node::Branch(BranchNode::from_bytes(data))),
                    other => Err(Error::new(format!("Wrong key prefix: {}", other))),
                }
            }
            None => Err(Error::new(format!("Unable to find node with db_key {:?}", db_key))),
        }
    }

    fn insert_leaf(&self, key: &BitSlice, value: V) -> Result<Hash, Error> {
        debug_assert!(key.is_leaf_key());

        let hash = Self::hash_leaf(key, &value);
        let db_key = key.to_db_key();
        let bytes = value.serialize();
        self.map.put(&db_key, bytes)?;
        Ok(hash)
    }

    fn insert_branch(&self, key: &BitSlice, branch: BranchNode) -> Result<(), Error> {
        let db_key = key.to_db_key();
        self.map.put(&db_key, branch.serialize())
    }

    // TODO replace by debug trait impl
    // fn print_node(&self, node: Node<V>) {
    //     match node {
    //         Node::Branch(branch) => {
    //             println!("{:#?}", branch);
    //             for i in [ChildKind::Left, ChildKind::Right].iter() {
    //                 let key = &branch.child_slice(*i);
    //                 println!("Key: {:#?}", key);
    //                 self.print_node(self.read_node(&key.to_db_key()).unwrap());
    //             }
    //         }
    //         Node::Leaf(data) => {
    //             println!("    Leaf: {:?}", data.serialize());
    //         }
    //     };
    // }

    // fn dump(&self) {
    //     println!("\n----- Dump MerklePatriciaTree -----\n");
    //     let root_node = self.root_node().unwrap();
    //     if let Some(node) = root_node {
    //         println!("Prefix: {:?}", node.0);
    //         println!("Root hash: {:#?}",
    //                  &bytes_to_hex(&self.root_hash().unwrap().unwrap()));
    //         self.print_node(node.1);
    //     }
    // }
}

impl<'a, T, K: ?Sized, V> Map<K, V> for MerklePatriciaTable<T, K, V>
    where T: Map<[u8], Vec<u8>>,
          K: AsRef<[u8]>,
          V: StorageValue
{
    fn get(&self, key: &K) -> Result<Option<V>, Error> {
        let key = BitSlice::from_bytes(key.as_ref()).to_db_key();        
        let v = self.map.get(key.as_ref())?;
        Ok(v.map(StorageValue::deserialize))
    }

    fn put(&self, key: &K, value: V) -> Result<(), Error> {
        // FIXME avoid reallocation
        self.insert(&key.as_ref().to_vec(), value)
    }

    fn delete(&self, key: &K) -> Result<(), Error> {
        self.remove(BitSlice::from_bytes(key.as_ref()))
    }

    fn find_key(&self, _: &K) -> Result<Option<Vec<u8>>, Error> {
        unimplemented!();
    }
}

// TODO move to utils mod
fn bytes_to_hex<T: AsRef<[u8]>>(bytes: &T) -> String {
    let strs: Vec<String> = bytes.as_ref()
        .iter()
        .map(|b| format!("{:02X}", b))
        .collect();
    strs.join("")
}

// fn bytes_to_binary<T: AsRef<[u8]>>(bytes: &T) -> String {
//     let strs: Vec<String> = bytes.as_ref()
//         .iter()
//         .map(|b| format!("{:b}", b))
//         .collect();
//     strs.join("")
// }

impl<'a> fmt::Debug for BitSlice<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let bytes = self.to_db_key();
        let key = &bytes[1..];
        if key.is_empty() {
            f.debug_struct("BitSlice").finish()
        } else if self.is_leaf_key() {
            f.debug_struct("BitSlice")
                .field("type", &"leaf")
                .field("key", &format!("0x{}", &bytes_to_hex(&key)))
                .field("from", &self.from)
                .field("to", &self.to)
                .finish()
        } else {
            f.debug_struct("BitSlice")
                .field("type", &"branch")
                .field("key", &format!("0x{}", &bytes_to_hex(&key)))
                .field("from", &self.from)
                .field("to", &self.to)
                .field("len", &key[32])
                .finish()
        }
    }
}

impl fmt::Debug for BranchNode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("BranchNode")
            .field("left_prefix", &self.child_slice(ChildKind::Left))
            .field("left_hash", &bytes_to_hex(self.child_hash(ChildKind::Left)))
            .field("right_prefix", &self.child_slice(ChildKind::Right))
            .field("right_hash",
                   &bytes_to_hex(self.child_hash(ChildKind::Right)))
            .field("hash", &bytes_to_hex(&self.hash()))
            .finish()
    }
}

// TODO add proper implementation based on debug_map
// impl<'a, T, K: ?Sized, V> fmt::Debug for MerklePatriciaTable<T, K, V>
//     where T: Map<[u8], Vec<u8>>,
//           K: AsRef<[u8]>,
//           V: StorageValue
// {
//     fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
//         // TODO add debug map
//         let mut b = f.debug_struct("MerklePatriciaTable");
//         b.field("root_prefix", &self.root_prefix().unwrap());
//         b.field("root_node", &self.root_node().unwrap());
//         b.finish()
//     }
// }

#[cfg(test)]
mod tests {
    extern crate rand;

    use std::collections::HashSet;

    use rand::{thread_rng, Rng};

    use ::crypto::{hash, Hash};
    use ::storage::{Map, MemoryDB, MapTable};

    use super::{BitSlice, BranchNode, MerklePatriciaTable};
    use super::ChildKind::{Left, Right};
    use super::KEY_SIZE;

    impl BranchNode {
        fn new(hashes: [&Hash; 2], prefixes: [&BitSlice; 2]) -> BranchNode {
            let mut node = Self::empty();
            unsafe {
                let mut cursor = node.write_hash(0, hashes[0]);
                cursor = node.write_hash(cursor, hashes[1]);
                cursor = node.write_slice(cursor, prefixes[0]);
                cursor = node.write_slice(cursor, prefixes[1]);
                debug_assert_eq!(cursor, 132);
            }
            node
        }
    }

    // Makes large data set with unique keys
    fn generate_random_data(len: usize) -> Vec<(Vec<u8>, Vec<u8>)> {
        let mut rng = thread_rng();

        let mut exists_keys = HashSet::new();
        let mut base: Vec<u8> = vec![0; KEY_SIZE];
        rng.fill_bytes(&mut base);
        let base = base;

        let kv_generator = |_| {
            let mut v = vec![0; 8];

            // Generate only unique keys
            let mut k = base.clone();
            let byte: usize = rng.gen_range(0, 31);
            k[byte] = rng.gen::<u8>();

            rng.fill_bytes(&mut v);
            while exists_keys.contains(&k) {
                rng.fill_bytes(&mut k);
            }
            exists_keys.insert(k.clone());
            (k, v)
        };

        (0..len)
            .map(kv_generator)
            .collect::<Vec<_>>()
    }

    #[test]
    fn bitslice_mid() {
        let v1 = vec![1, 2, 255, 12];
        let b = BitSlice::from_bytes(&v1);

        assert_eq!(b.len(), 32);

        assert_eq!(b.at(0), Left);
        assert_eq!(b.at(7), Right);
        assert_eq!(b.at(8), Left);
        assert_eq!(b.at(14), Right);
        assert_eq!(b.at(15), Left);
        assert_eq!(b.at(16), Right);
        assert_eq!(b.at(20), Right);
        assert_eq!(b.at(23), Right);
        assert_eq!(b.at(31), Left);

        let b2 = b.mid(8);
        assert_eq!(b2.len(), 24);
        assert_eq!(b2.at(0), Left);
        assert_eq!(b2.at(6), Right);
        assert_eq!(b2.at(7), Left);
        assert_eq!(b2.at(12), Right);
        assert_eq!(b2.at(15), Right);

        let b3 = b2.mid(24);
        assert_eq!(b3.len(), 0);
    }

    #[test]
    fn bitslice_truncate() {
        let v1 = vec![128];
        let b = BitSlice::from_bytes(&v1);

        assert_eq!(b.len(), 8);
        assert_eq!(b.truncate(1).at(0), Right);
    }

    #[test]
    fn bitslice_len() {
        let v1 = vec![255;32];
        let b = BitSlice::from_bytes(&v1);

        assert_eq!(b.len(), 256);
    }

    #[test]
    #[should_panic]
    fn bitslice_at_overflow() {
        let v1 = vec![8, 16, 32, 64];
        let b = BitSlice::from_bytes(&v1);
        b.at(32);
    }

    #[test]
    #[should_panic]
    fn bitslice_mid_overflow() {
        let v1 = vec![255;32];
        let b = BitSlice::from_bytes(&v1);
        b.mid(255).mid(255);
    }

    #[test]
    #[should_panic]
    fn bitslice_mid_at_overflow() {
        let v1 = vec![255;32];
        let b = BitSlice::from_bytes(&v1);
        b.mid(1).at(255);
    }

    #[test]
    fn bitslice_common_prefix() {
        let b1 = BitSlice::from_bytes(b"abcd");
        let b2 = BitSlice::from_bytes(b"abde");
        assert_eq!(b1.common_prefix(&b1), 32);

        let c = b1.common_prefix(&b2);
        assert_eq!(c, 21);
        let c = b2.common_prefix(&b1);
        assert_eq!(c, 21);

        let b1 = b1.mid(9);
        let b2 = b2.mid(9);
        let c = b1.common_prefix(&b2);
        assert_eq!(c, 12);

        let v1 = &vec![255; 32];
        let v2 = &vec![254; 32];
        let b3 = BitSlice::from_bytes(&v1);
        let b4 = BitSlice::from_bytes(&v2);
        assert_eq!(b3.common_prefix(&b4), 7);
        assert_eq!(b4.common_prefix(&b3), 7);
        assert_eq!(b3.common_prefix(&b3), 256);
        let b3 = b3.mid(30);
        assert_eq!(b3.common_prefix(&b3), 226);
        let b3 = b3.truncate(200);
        assert_eq!(b3.common_prefix(&b3), 200);
    }

    #[test]
    fn bitslice_to_db_key() {
        let b = BitSlice::from_bytes(b"baceaaaaaaaaaaaaaaaaaaaaaaaaaaaa");

        assert_eq!(b.to_db_key(),
                   vec![1, 98, 97, 99, 101, 97, 97, 97, 97, 97, 97, 97, 97, 97, 97, 97, 97, 97,
                        97, 97, 97, 97, 97, 97, 97, 97, 97, 97, 97, 97, 97, 97, 97, 0]);
        assert_eq!(b.truncate(16).to_db_key(),
                   vec![0, 98, 97, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                        0, 0, 0, 0, 0, 0, 0, 0, 0, 16]);
        assert_eq!(b.truncate(31).to_db_key(),
                   vec![0, 98, 97, 99, 100, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31]);
        assert_eq!(b.mid(8).truncate(8).to_db_key(),
                   vec![0, 98, 97, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                        0, 0, 0, 0, 0, 0, 0, 0, 0, 16]);
        assert_eq!(b.mid(8).to_db_key(),
                   vec![1, 98, 97, 99, 101, 97, 97, 97, 97, 97, 97, 97, 97, 97, 97, 97, 97, 97,
                        97, 97, 97, 97, 97, 97, 97, 97, 97, 97, 97, 97, 97, 97, 97, 0]);
        assert_eq!(b.mid(12).to_db_key(),
                   vec![1, 98, 97, 99, 101, 97, 97, 97, 97, 97, 97, 97, 97, 97, 97, 97, 97, 97,
                        97, 97, 97, 97, 97, 97, 97, 97, 97, 97, 97, 97, 97, 97, 97, 0]);
        assert_eq!(b.mid(12).truncate(4).to_db_key(),
                   vec![0, 98, 97, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                        0, 0, 0, 0, 0, 0, 0, 0, 0, 16]);
        assert_eq!(b.mid(12).truncate(19).to_db_key(),
                   vec![0, 98, 97, 99, 100, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31]);

        let b = BitSlice::from_bytes(b"p");
        assert_eq!(b.mid(4).to_db_key(),
                   vec![1, 112, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                        0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn bitslice_from_db_key() {
        let b = BitSlice::from_bytes(b"qwertyuiopasdfghjklzxcvbnm123456");
        assert_eq!(b.len(), 256);
        assert_eq!(b.mid(4),
                   BitSlice::from_db_key(&b.mid(4).to_db_key()).mid(4));
        assert_eq!(b.truncate(24),
                   BitSlice::from_db_key(&b.truncate(24).to_db_key()));
    }

    #[test]
    fn bitslice_is_leaf_key() {
        let b = BitSlice::from_bytes(b"qwertyuiopasdfghjklzxcvbnm123456");
        assert_eq!(b.len(), 256);
        assert_eq!(b.mid(4).is_leaf_key(), true);
        assert_eq!(b.mid(8).is_leaf_key(), true);
        assert_eq!(b.mid(250).is_leaf_key(), true);
        assert_eq!(b.truncate(16).is_leaf_key(), false);
    }

    #[test]
    fn branch_node() {
        let mut rng = thread_rng();
        let mut gen_seq = || rng.gen_iter::<u8>().take(32).collect::<Vec<u8>>();

        let lh = Hash::from_slice(&gen_seq()).unwrap();
        let rh = Hash::from_slice(&gen_seq()).unwrap();
        let lp = BitSlice {
            data: &gen_seq(),
            from: 0,
            to: 16,
        };
        let rp = BitSlice {
            data: &gen_seq(),
            from: 31,
            to: 64,
        };
        let node = BranchNode::new([&lh, &rh], [&lp, &rp]);

        assert_eq!(node.child_hash(Left), &lh);
        assert_eq!(node.child_hash(Right), &rh);
        assert_eq!(node.child_slice(Left).data, lp.data);
        assert_eq!(node.child_slice(Left).to, lp.to);
        assert_eq!(node.child_slice(Right).data, rp.data);
        assert_eq!(node.child_slice(Right).to, rp.to);
        // assert_eq!(node.child_db_key(Left), lp.to_db_key().as_slice());
        // assert_eq!(node.child_db_key(Right), rp.to_db_key().as_slice());
    }

    #[test]
    fn insert_trivial() {
        let mut storage1 = MemoryDB::new();
        let mut storage2 = MemoryDB::new();
        let map1 = MapTable::new(vec![255], &mut storage1);
        let map2 = MapTable::new(vec![255], &mut storage2);

        let table1 = MerklePatriciaTable::new(map1);
        table1.put(&vec![255; 32], vec![1]).unwrap();
        table1.put(&vec![254; 32], vec![2]).unwrap();

        let table2 = MerklePatriciaTable::new(map2);
        table2.put(&vec![254; 32], vec![2]).unwrap();
        table2.put(&vec![255; 32], vec![1]).unwrap();

        assert_eq!(table1.get(&vec![255; 32]).unwrap(), Some(vec![1]));
        assert_eq!(table1.get(&vec![254; 32]).unwrap(), Some(vec![2]));
        assert_eq!(table2.get(&vec![255; 32]).unwrap(), Some(vec![1]));
        assert_eq!(table2.get(&vec![254; 32]).unwrap(), Some(vec![2]));

        assert!(table1.root_hash().unwrap() != Hash([0; 32]));
        assert_eq!(table1.root_hash().unwrap(), table2.root_hash().unwrap());
    }

    #[test]
    fn insert_same_key() {
        let mut storage = MemoryDB::new();
        let map = MapTable::new(vec![255], &mut storage);
        let table = MerklePatriciaTable::new(map);
        let hash = hash([vec![255; 32].as_ref(), hash(&vec![2]).as_ref()].concat().as_ref());

        table.put(&vec![255; 32], vec![1]).unwrap();
        table.put(&vec![255; 32], vec![2]).unwrap();
        assert_eq!(table.get(&vec![255; 32]).unwrap(), Some(vec![2]));
        assert_eq!(table.root_hash().unwrap(), hash);
    }

    #[test]
    fn insert_simple() {
        let mut storage1 = MemoryDB::new();
        let mut storage2 = MemoryDB::new();
        let map1 = MapTable::new(vec![255], &mut storage1);
        let map2 = MapTable::new(vec![255], &mut storage2);

        let table1 = MerklePatriciaTable::new(map1);
        table1.put(&vec![255; 32], vec![3]).unwrap();
        table1.put(&vec![254; 32], vec![2]).unwrap();
        table1.put(&vec![250; 32], vec![1]).unwrap();
        table1.put(&vec![254; 32], vec![5]).unwrap();

        let table2 = MerklePatriciaTable::new(map2);
        table2.put(&vec![250; 32], vec![1]).unwrap();
        table2.put(&vec![254; 32], vec![2]).unwrap();
        table2.put(&vec![255; 32], vec![3]).unwrap();
        table2.put(&vec![254; 32], vec![5]).unwrap();

        assert!(table1.root_hash().unwrap() != Hash([0; 32]));
        assert_eq!(table1.root_hash().unwrap(), table2.root_hash().unwrap());
    }

    #[test]
    fn insert_reverse() {
        let mut storage1 = MemoryDB::new();
        let map1 = MapTable::new(vec![255], &mut storage1);
        let table1 = MerklePatriciaTable::new(map1);
        table1.put(&vec![42; 32], vec![1]).unwrap();
        table1.put(&vec![64; 32], vec![2]).unwrap();
        table1.put(&vec![240; 32], vec![3]).unwrap();
        table1.put(&vec![245; 32], vec![4]).unwrap();
        table1.put(&vec![250; 32], vec![5]).unwrap();
        table1.put(&vec![255; 32], vec![6]).unwrap();

        let mut storage2 = MemoryDB::new();
        let map2 = MapTable::new(vec![255], &mut storage2);
        let table2 = MerklePatriciaTable::new(map2);
        table2.put(&vec![255; 32], vec![6]).unwrap();
        table2.put(&vec![250; 32], vec![5]).unwrap();
        table2.put(&vec![245; 32], vec![4]).unwrap();
        table2.put(&vec![240; 32], vec![3]).unwrap();
        table2.put(&vec![64; 32], vec![2]).unwrap();
        table2.put(&vec![42; 32], vec![1]).unwrap();

        assert!(table2.root_hash().unwrap() != Hash([0; 32]));
        assert_eq!(table2.root_hash().unwrap(), table1.root_hash().unwrap());
    }

    #[test]
    fn remove_trivial() {
        let mut storage1 = MemoryDB::new();
        let map1 = MapTable::new(vec![255], &mut storage1);
        let table1 = MerklePatriciaTable::new(map1);
        table1.put(&vec![255; 32], vec![6]).unwrap();
        table1.delete(&vec![255; 32]).unwrap();

        let mut storage2 = MemoryDB::new();
        let map2 = MapTable::new(vec![255], &mut storage2);
        let table2 = MerklePatriciaTable::new(map2);
        table2.put(&vec![255; 32], vec![6]).unwrap();
        table2.delete(&vec![255; 32]).unwrap();

        assert_eq!(table1.root_hash().unwrap(), Hash([0; 32]));
        assert_eq!(table2.root_hash().unwrap(), Hash([0; 32]));
    }

    #[test]
    fn remove_simple() {
        let mut storage1 = MemoryDB::new();
        let map1 = MapTable::new(vec![255], &mut storage1);
        let table1 = MerklePatriciaTable::new(map1);
        table1.put(&vec![255; 32], vec![1]).unwrap();
        table1.put(&vec![250; 32], vec![2]).unwrap();
        table1.put(&vec![245; 32], vec![3]).unwrap();

        table1.delete(&vec![255; 32]).unwrap();
        table1.delete(&vec![245; 32]).unwrap();

        let mut storage2 = MemoryDB::new();
        let map2 = MapTable::new(vec![255], &mut storage2);
        let table2 = MerklePatriciaTable::new(map2);
        table2.put(&vec![250; 32], vec![2]).unwrap();
        table2.put(&vec![255; 32], vec![1]).unwrap();
        table2.put(&vec![245; 32], vec![3]).unwrap();

        table2.delete(&vec![255; 32]).unwrap();
        table2.delete(&vec![245; 32]).unwrap();

        assert_eq!(table2.get(&vec![250; 32]).unwrap(), Some(vec![2]));
        assert_eq!(table1.get(&vec![250; 32]).unwrap(), Some(vec![2]));

        assert_eq!(table1.root_hash().unwrap(), table2.root_hash().unwrap());
    }

    #[test]
    fn remove_reverse() {
        let mut storage1 = MemoryDB::new();
        let map1 = MapTable::new(vec![255], &mut storage1);
        let table1 = MerklePatriciaTable::new(map1);
        table1.put(&vec![42; 32], vec![1]).unwrap();
        table1.put(&vec![64; 32], vec![2]).unwrap();
        table1.put(&vec![240; 32], vec![3]).unwrap();
        table1.put(&vec![245; 32], vec![4]).unwrap();
        table1.put(&vec![250; 32], vec![5]).unwrap();
        table1.put(&vec![255; 32], vec![6]).unwrap();

        table1.delete(&vec![255; 32]).unwrap();
        table1.delete(&vec![250; 32]).unwrap();
        table1.delete(&vec![245; 32]).unwrap();
        table1.delete(&vec![240; 32]).unwrap();
        table1.delete(&vec![64; 32]).unwrap();
        table1.delete(&vec![42; 32]).unwrap();

        let mut storage2 = MemoryDB::new();
        let map2 = MapTable::new(vec![255], &mut storage2);
        let table2 = MerklePatriciaTable::new(map2);
        table2.put(&vec![255; 32], vec![6]).unwrap();
        table2.put(&vec![250; 32], vec![5]).unwrap();
        table2.put(&vec![245; 32], vec![4]).unwrap();
        table2.put(&vec![240; 32], vec![3]).unwrap();
        table2.put(&vec![64; 32], vec![2]).unwrap();
        table2.put(&vec![42; 32], vec![1]).unwrap();

        table2.delete(&vec![42; 32]).unwrap();
        table2.delete(&vec![64; 32]).unwrap();
        table2.delete(&vec![240; 32]).unwrap();
        table2.delete(&vec![245; 32]).unwrap();
        table2.delete(&vec![250; 32]).unwrap();
        table2.delete(&vec![255; 32]).unwrap();

        assert_eq!(table2.root_hash().unwrap(), table1.root_hash().unwrap());
    }

    #[test]
    fn fuzz_insert() {
        let mut data = generate_random_data(100);
        let mut rng = rand::thread_rng();

        let mut storage1 = MemoryDB::new();
        let map1 = MapTable::new(vec![255], &mut storage1);
        let table1 = MerklePatriciaTable::new(map1);
        for item in &data {
            table1.put(&item.0, item.1.clone()).unwrap();
        }

        let mut storage2 = MemoryDB::new();
        let map2 = MapTable::new(vec![255], &mut storage2);
        let table2 = MerklePatriciaTable::new(map2);
        rng.shuffle(&mut data);
        for item in &data {
            table2.put(&item.0, item.1.clone()).unwrap();
        }

        for item in &data {
            let v1 = table1.get(&item.0).unwrap();
            let v2 = table2.get(&item.0).unwrap();
            assert_eq!(v1.as_ref(), Some(&item.1));
            assert_eq!(v2.as_ref(), Some(&item.1));
        }

        assert!(table2.root_hash().unwrap() != Hash([0; 32]));
        assert_eq!(table2.root_hash().unwrap(), table1.root_hash().unwrap());

        // Test same keys
        rng.shuffle(&mut data);
        for item in &data {
            table1.put(&item.0, vec![1]).unwrap();
        }
        rng.shuffle(&mut data);
        for item in &data {
            table2.put(&item.0, vec![1]).unwrap();
        }

        for item in &data {
            let v1 = table1.get(&item.0).unwrap();
            let v2 = table2.get(&item.0).unwrap();
            assert_eq!(v1.as_ref(), Some(&vec![1]));
            assert_eq!(v2.as_ref(), Some(&vec![1]));
        }
        assert_eq!(table2.root_hash().unwrap(), table1.root_hash().unwrap());
    }

    #[test]
    fn fuzz_delete() {
        let mut data = generate_random_data(100);
        let mut rng = rand::thread_rng();

        let mut storage1 = MemoryDB::new();
        let map1 = MapTable::new(vec![255], &mut storage1);
        let table1 = MerklePatriciaTable::new(map1);
        for item in &data {
            table1.put(&item.0, item.1.clone()).unwrap();
        }

        let mut storage2 = MemoryDB::new();
        let map2 = MapTable::new(vec![255], &mut storage2);
        let table2 = MerklePatriciaTable::new(map2);
        rng.shuffle(&mut data);
        for item in &data {
            table2.put(&item.0, item.1.clone()).unwrap();
        }

        let saved_hash = table1.root_hash().unwrap();

        let mut keys_to_remove = data.iter()
            .take(50)
            .map(|item| item.0.clone())
            .collect::<Vec<_>>();

        rng.shuffle(&mut keys_to_remove);
        for key in &keys_to_remove {
            table1.delete(key).unwrap();
        }
        rng.shuffle(&mut keys_to_remove);
        for key in &keys_to_remove {
            table2.delete(key).unwrap();
        }

        assert!(table2.root_hash().unwrap() != Hash([0; 32]));
        assert_eq!(table2.root_hash().unwrap(), table1.root_hash().unwrap());

        for item in &data {
            table1.put(&item.0, item.1.clone()).unwrap();
        }
        rng.shuffle(&mut data);
        for item in &data {
            table2.put(&item.0, item.1.clone()).unwrap();
        }

        for item in &data {
            let v1 = table1.get(&item.0).unwrap();
            let v2 = table2.get(&item.0).unwrap();
            assert_eq!(v1.as_ref(), Some(&item.1));
            assert_eq!(v2.as_ref(), Some(&item.1));
        }
        assert_eq!(table2.root_hash().unwrap(), table1.root_hash().unwrap());
        assert_eq!(table2.root_hash().unwrap(), saved_hash);
    }

    #[test]
    fn fuzz_insert_after_delete() {
        let mut storage1 = MemoryDB::new();
        let map1 = MapTable::new(vec![255], &mut storage1);
        let table1 = MerklePatriciaTable::new(map1);

        let data = generate_random_data(100);

        for item in &data[0..50] {
            table1.put(&item.0, item.1.clone()).unwrap();
        }
        let saved_hash = table1.root_hash().unwrap();
        for item in &data[50..] {
            table1.put(&item.0, item.1.clone()).unwrap();
        }
        for item in &data[50..] {
            table1.delete(&item.0).unwrap();
        }

        for item in &data[0..50] {
            let v1 = table1.get(&item.0).unwrap();
            assert_eq!(v1.as_ref(), Some(&item.1));
        }
        for item in &data[50..] {
            let v1 = table1.get(&item.0).unwrap();
            assert_eq!(v1.as_ref(), None);
        }
        assert_eq!(table1.root_hash().unwrap(), saved_hash);
    }
}
