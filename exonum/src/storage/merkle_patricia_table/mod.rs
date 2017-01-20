mod proofpathtokey;
use std::mem;
use std::cmp::{min, PartialEq};
use std::marker::PhantomData;
use std::fmt;
use std::ops::Not;
use super::utils::bytes_to_hex;

use ::crypto::{hash, Hash, HASH_SIZE};
use super::{Map, Error, StorageValue};
use self::proofpathtokey::{RootProofNode, BranchProofNode, ProofNode, BitVec};

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
#[derive(Clone)]
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
        hash(self.raw.as_slice())
        // which is effectively the same as written below
        // === hash(&[self.child_hash(ChildKind::Left).as_ref(),
        // self.child_hash(ChildKind::Right).as_ref(),
        // &self.child_slice(ChildKind::Left).to_db_key(),
        // &self.child_slice(ChildKind::Right).to_db_key()]
        // .concat())

        // as it used to be:
        // hash(&[self.child_hash(ChildKind::Left).as_ref(),
        //        self.child_hash(ChildKind::Right).as_ref()]
        //     .concat())
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
        debug_assert!(slice.data.len() == KEY_SIZE);

        let db_key = slice.to_db_key();

        self.raw[from..from + DB_KEY_SIZE].copy_from_slice(db_key.as_slice());
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

type Entry<V> = (Vec<u8>, Node<V>);

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
            Some((root_db_key, Node::Leaf(value))) => {
                Ok(hash(&[root_db_key.as_slice(), value.hash().as_ref()].concat()))
            }
            Some((_, Node::Branch(branch))) => Ok(branch.hash()),
            None => Ok(Hash::zero()),
        }
    }

    fn root_node(&self) -> Result<Option<Entry<V>>, Error> {
        let out = match self.root_prefix()? {
            Some(db_key) => {
                let node = self.read_node(&db_key)?;
                Some((db_key, node))
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
                                     &prefix_data.hash());
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

    // fn hash_leaf(value: &V) -> Hash {
    //     value.hash()
    // }

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

    pub fn construct_path_to_key<A: AsRef<[u8]>>(&self,
                                                 searched_key: A)
                                                 -> Result<RootProofNode<V>, Error> {
        let searched_key = searched_key.as_ref();
        debug_assert_eq!(searched_key.len(), KEY_SIZE);
        let searched_slice = BitSlice::from_bytes(searched_key);
        let suff_from = 0;

        let res: RootProofNode<V> = match self.root_node()? {
            Some((root_db_key, Node::Leaf(root_value))) => {
                if searched_slice.to_db_key() == root_db_key {
                    RootProofNode::LeafRootInclusive(BitVec::new(root_db_key,
                                                                 suff_from,
                                                                 KEY_SIZE * 8),
                                                     root_value)
                } else {
                    RootProofNode::LeafRootExclusive(BitVec::new(root_db_key,
                                                                 suff_from,
                                                                 KEY_SIZE * 8),
                                                     root_value.hash())
                }
            } 
            Some((root_db_key, Node::Branch(branch))) => {
                let root_slice = BitSlice::from_db_key(&root_db_key);
                let l_s = branch.child_slice(ChildKind::Left);
                let r_s = branch.child_slice(ChildKind::Right);
                let l_s_db_key = l_s.to_db_key();
                let r_s_db_key = r_s.to_db_key();

                let c_pr_l = root_slice.common_prefix(&searched_slice);
                if c_pr_l == root_slice.len() {
                    let suf_searched_slice = searched_slice.mid(c_pr_l);
                    let proof_from_level_below: Option<ProofNode<V>> =
                        self.construct_path_to_key_in_branch(&branch, &suf_searched_slice)?;

                    if let Some(child_proof) = proof_from_level_below {
                        let child_proof_pos = suf_searched_slice.at(0);
                        let neighbour_child_hash = *branch.child_hash(!child_proof_pos);
                        match child_proof_pos {
                            ChildKind::Left => {
                                RootProofNode::Branch(BranchProofNode::LeftBranch {
                                    left_hash: Box::new(child_proof),
                                    right_hash: neighbour_child_hash,
                                    left_key: BitVec::new(l_s_db_key, suff_from, l_s.to as usize),
                                    right_key: BitVec::new(r_s_db_key, suff_from, r_s.to as usize),
                                })
                            } 
                            ChildKind::Right => {
                                RootProofNode::Branch(BranchProofNode::RightBranch {
                                    left_hash: neighbour_child_hash,
                                    right_hash: Box::new(child_proof),
                                    left_key: BitVec::new(l_s_db_key, suff_from, l_s.to as usize),
                                    right_key: BitVec::new(r_s_db_key, suff_from, r_s.to as usize),
                                })
                            }
                        }
                    } else {
                        let l_h = *branch.child_hash(ChildKind::Left); //copy
                        let r_h = *branch.child_hash(ChildKind::Right);//copy
                        RootProofNode::Branch(BranchProofNode::BranchKeyNotFound {
                            left_hash: l_h,
                            right_hash: r_h,
                            left_key: BitVec::new(l_s_db_key, suff_from, l_s.to as usize),
                            right_key: BitVec::new(r_s_db_key, suff_from, r_s.to as usize),
                        })
                        // proof of exclusion of a key, because none of child slices is a prefix(searched_slice)
                    }
                } else {
                    // if common prefix length with root_slice is less than root_slice length
                    let l_h = *branch.child_hash(ChildKind::Left); //copy
                    let r_h = *branch.child_hash(ChildKind::Right);//copy
                    RootProofNode::Branch(BranchProofNode::BranchKeyNotFound {
                        left_hash: l_h,
                        right_hash: r_h,
                        left_key: BitVec::new(l_s_db_key, suff_from, l_s.to as usize),
                        right_key: BitVec::new(r_s_db_key, suff_from, r_s.to as usize),
                    })
                    // proof of exclusion of a key, because root_slice != prefix(searched_slice)
                }
            } 
            None => return Ok(RootProofNode::Empty),
        };
        Ok(res)
    }

    fn construct_path_to_key_in_branch(&self,
                                       current_branch: &BranchNode,
                                       searched_slice: &BitSlice)
                                       -> Result<Option<ProofNode<V>>, Error> {

        let mut child_slice = current_branch.child_slice(searched_slice.at(0));
        child_slice.from = searched_slice.from;
        let c_pr_l = child_slice.common_prefix(searched_slice);
        let suff_from = (searched_slice.from as usize) + c_pr_l;
        debug_assert!(c_pr_l > 0);
        if c_pr_l < child_slice.len() {
            return Ok(None);
        }

        let res: ProofNode<V> = match self.read_node(child_slice.to_db_key())? {
            Node::Leaf(child_value) => ProofNode::Leaf(child_value), 
            Node::Branch(child_branch) => {
                let l_s = child_branch.child_slice(ChildKind::Left);
                let r_s = child_branch.child_slice(ChildKind::Right);
                let l_s_db_key = l_s.to_db_key();
                let r_s_db_key = r_s.to_db_key();
                let suf_searched_slice = searched_slice.mid(c_pr_l);
                let proof_from_level_below: Option<ProofNode<V>> =
                    self.construct_path_to_key_in_branch(&child_branch, &suf_searched_slice)?;

                if let Some(child_proof) = proof_from_level_below {
                    let child_proof_pos = suf_searched_slice.at(0);
                    let neighbour_child_hash = *child_branch.child_hash(!child_proof_pos);
                    match child_proof_pos {
                        ChildKind::Left => {
                            ProofNode::Branch(BranchProofNode::LeftBranch {
                                left_hash: Box::new(child_proof),
                                right_hash: neighbour_child_hash,
                                left_key: BitVec::new(l_s_db_key, suff_from, l_s.to as usize),
                                right_key: BitVec::new(r_s_db_key, suff_from, r_s.to as usize),
                            })
                        }
                        ChildKind::Right => {
                            ProofNode::Branch(BranchProofNode::RightBranch {
                                left_hash: neighbour_child_hash,
                                right_hash: Box::new(child_proof),
                                left_key: BitVec::new(l_s_db_key, suff_from, l_s.to as usize),
                                right_key: BitVec::new(r_s_db_key, suff_from, r_s.to as usize),
                            })
                        } 
                    }
                } else {
                    let l_h = *child_branch.child_hash(ChildKind::Left); //copy
                    let r_h = *child_branch.child_hash(ChildKind::Right);//copy
                    ProofNode::Branch(BranchProofNode::BranchKeyNotFound {
                        left_hash: l_h,
                        right_hash: r_h,
                        left_key: BitVec::new(l_s_db_key, suff_from, l_s.to as usize),
                        right_key: BitVec::new(r_s_db_key, suff_from, r_s.to as usize),
                    })
                    // proof of exclusion of a key, because none of child slices is a prefix(searched_slice)
                }
            }
        };
        Ok(Some(res))
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

        let hash = value.hash();
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
        let db_key = BitSlice::from_bytes(key.as_ref()).to_db_key();
        let v = self.map.get(db_key.as_ref())?;
        Ok(v.map(StorageValue::deserialize))
    }

    fn put(&self, key: &K, value: V) -> Result<(), Error> {
        // FIXME avoid reallocation
        self.insert(key, value)
    }

    fn delete(&self, key: &K) -> Result<(), Error> {
        self.remove(BitSlice::from_bytes(key.as_ref()))
    }

    fn find_key(&self, key: &K) -> Result<Option<Vec<u8>>, Error> {
        let key = key.as_ref();
        debug_assert!(key.len() <= KEY_SIZE);

        let mut db_key = vec![0; DB_KEY_SIZE];
        db_key[0] = LEAF_KEY_PREFIX;
        db_key[1..key.len() + 1].copy_from_slice(key);

        let r = self.map.find_key(db_key.as_slice())?;
        Ok(r.map(|v| v[1..v.len() - 1].to_vec()))
    }
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
        let bytes_hex = bytes_to_hex(&self.data);
        let repr_string = format!("0x{}_from:{}_to:{}", bytes_hex, self.from, self.to);
        write!(f, "{:?}", repr_string)
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
    use ::storage::utils::bytes_to_hex;
    use serde_json;

    use super::{BitSlice, BranchNode, MerklePatriciaTable, LEAF_KEY_PREFIX};
    use super::proofpathtokey::{RootProofNode/*, verify_proof_consistency*/};
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

    // Makes large data set with unique keys
    fn generate_fully_random_data_keys(len: usize) -> Vec<(Vec<u8>, Vec<u8>)> {
        let mut rng = thread_rng();

        let mut exists_keys = HashSet::new();

        let kv_generator = |_| {
            let mut v = vec![0; 8];
            let mut new_key: Vec<u8> = vec![0; KEY_SIZE];
            rng.fill_bytes(&mut new_key);

            while exists_keys.contains(&new_key) {
                rng.fill_bytes(&mut new_key);
            }
            exists_keys.insert(new_key.clone());
            rng.fill_bytes(&mut v);
            (new_key, v)
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
        assert_eq!(b.truncate(1).len(), 1);
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
        b.mid(255).mid(2);
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
        let b3 = BitSlice::from_bytes(v1);
        let b4 = BitSlice::from_bytes(v2);
        assert_eq!(b3.common_prefix(&b4), 7);
        assert_eq!(b4.common_prefix(&b3), 7);
        assert_eq!(b3.common_prefix(&b3), 256);
        let b3 = b3.mid(30);
        assert_eq!(b3.common_prefix(&b3), 226);
        let b3 = b3.truncate(200);
        assert_eq!(b3.common_prefix(&b3), 200);
        let b5 = BitSlice::from_bytes(v1);
        assert_eq!(b5.truncate(0).common_prefix(&b3), 0);
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
        assert_eq!(node.child_slice(Left).to_db_key(), lp.to_db_key());
        assert_eq!(node.child_slice(Left).to, lp.to);
        assert_eq!(node.child_slice(Right).to_db_key(), rp.to_db_key());
        assert_eq!(node.child_slice(Right).to, rp.to);
        // assert_eq!(node.child_db_key(Left), lp.to_db_key().as_slice());
        // assert_eq!(node.child_db_key(Right), rp.to_db_key().as_slice());
    }

    #[test]
    fn insert_trivial() {
        let storage1 = MemoryDB::new();
        let storage2 = MemoryDB::new();
        let map1 = MapTable::new(vec![255], &storage1);
        let map2 = MapTable::new(vec![255], &storage2);

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

        // assert_eq!(table1.find_key(&vec![]).unwrap(), Some(vec![254; 32])); //FIXME
        assert_eq!(table1.find_key(&vec![254; 32]).unwrap(),
                   Some(vec![254; 32]));
        assert_eq!(table1.find_key(&vec![255; 32]).unwrap(),
                   Some(vec![255; 32]));

        assert!(table1.root_hash().unwrap() != Hash::zero());
        assert_eq!(table1.root_hash().unwrap(), table2.root_hash().unwrap());
    }

    #[test]
    fn insert_same_key() {
        let storage = MemoryDB::new();
        let map = MapTable::new(vec![255], &storage);
        let table = MerklePatriciaTable::new(map);
        assert_eq!(table.root_hash().unwrap(), Hash::zero());
        let root_prefix = &[&[LEAF_KEY_PREFIX], vec![255; 32].as_slice(), &[0u8]].concat();
        let hash = hash(&[root_prefix, hash(&[2]).as_ref()].concat());

        table.put(&vec![255; 32], vec![1]).unwrap();
        table.put(&vec![255; 32], vec![2]).unwrap();
        assert_eq!(table.get(&vec![255; 32]).unwrap(), Some(vec![2]));
        assert_eq!(table.root_hash().unwrap(), hash);
    }

    #[test]
    fn insert_simple() {
        let storage1 = MemoryDB::new();
        let storage2 = MemoryDB::new();
        let map1 = MapTable::new(vec![255], &storage1);
        let map2 = MapTable::new(vec![255], &storage2);

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

        assert!(table1.root_hash().unwrap() != Hash::zero());
        assert_eq!(table1.root_hash().unwrap(), table2.root_hash().unwrap());
    }

    #[test]
    fn insert_reverse() {
        let storage1 = MemoryDB::new();
        let map1 = MapTable::new(vec![255], &storage1);
        let table1 = MerklePatriciaTable::new(map1);
        table1.put(&vec![42; 32], vec![1]).unwrap();
        table1.put(&vec![64; 32], vec![2]).unwrap();
        table1.put(&vec![240; 32], vec![3]).unwrap();
        table1.put(&vec![245; 32], vec![4]).unwrap();
        table1.put(&vec![250; 32], vec![5]).unwrap();
        table1.put(&vec![255; 32], vec![6]).unwrap();

        let storage2 = MemoryDB::new();
        let map2 = MapTable::new(vec![255], &storage2);
        let table2 = MerklePatriciaTable::new(map2);
        table2.put(&vec![255; 32], vec![6]).unwrap();
        table2.put(&vec![250; 32], vec![5]).unwrap();
        table2.put(&vec![245; 32], vec![4]).unwrap();
        table2.put(&vec![240; 32], vec![3]).unwrap();
        table2.put(&vec![64; 32], vec![2]).unwrap();
        table2.put(&vec![42; 32], vec![1]).unwrap();


        assert!(table2.root_hash().unwrap() != Hash::zero());
        assert_eq!(table2.root_hash().unwrap(), table1.root_hash().unwrap());
    }

    #[test]
    fn remove_trivial() {
        let storage1 = MemoryDB::new();
        let map1 = MapTable::new(vec![255], &storage1);
        let table1 = MerklePatriciaTable::new(map1);
        table1.put(&vec![255; 32], vec![6]).unwrap();
        table1.delete(&vec![255; 32]).unwrap();

        let storage2 = MemoryDB::new();
        let map2 = MapTable::new(vec![255], &storage2);
        let table2 = MerklePatriciaTable::new(map2);
        table2.put(&vec![255; 32], vec![6]).unwrap();
        table2.delete(&vec![255; 32]).unwrap();

        assert_eq!(table1.root_hash().unwrap(), Hash::zero());
        assert_eq!(table2.root_hash().unwrap(), Hash::zero());
    }

    #[test]
    fn remove_simple() {
        let storage1 = MemoryDB::new();
        let map1 = MapTable::new(vec![255], &storage1);
        let table1 = MerklePatriciaTable::new(map1);
        table1.put(&vec![255; 32], vec![1]).unwrap();
        table1.put(&vec![250; 32], vec![2]).unwrap();
        table1.put(&vec![245; 32], vec![3]).unwrap();

        table1.delete(&vec![255; 32]).unwrap();
        table1.delete(&vec![245; 32]).unwrap();

        let storage2 = MemoryDB::new();
        let map2 = MapTable::new(vec![255], &storage2);
        let table2 = MerklePatriciaTable::new(map2);
        table2.put(&vec![250; 32], vec![2]).unwrap();
        table2.put(&vec![255; 32], vec![1]).unwrap();
        table2.put(&vec![245; 32], vec![3]).unwrap();

        table2.delete(&vec![255; 32]).unwrap();
        table2.delete(&vec![245; 32]).unwrap();

        assert_eq!(table2.get(&vec![250; 32]).unwrap(), Some(vec![2]));
        assert_eq!(table1.get(&vec![250; 32]).unwrap(), Some(vec![2]));
        assert!(table1.get(&vec![255; 32]).unwrap().is_none());
        assert!(table2.get(&vec![255; 32]).unwrap().is_none());
        assert!(table1.get(&vec![245; 32]).unwrap().is_none());
        assert!(table2.get(&vec![245; 32]).unwrap().is_none());

        assert_eq!(table1.root_hash().unwrap(), table2.root_hash().unwrap());
    }

    #[test]
    fn remove_reverse() {
        let storage1 = MemoryDB::new();
        let map1 = MapTable::new(vec![255], &storage1);
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

        let storage2 = MemoryDB::new();
        let map2 = MapTable::new(vec![255], &storage2);
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

        let storage1 = MemoryDB::new();
        let map1 = MapTable::new(vec![255], &storage1);
        let table1 = MerklePatriciaTable::new(map1);
        for item in &data {
            table1.put(&item.0, item.1.clone()).unwrap();
        }

        let storage2 = MemoryDB::new();
        let map2 = MapTable::new(vec![255], &storage2);
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

        assert!(table2.root_hash().unwrap() != Hash::zero());
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
    fn build_proof_in_empty_tree() {
        let storage = MemoryDB::new();
        let map = MapTable::new(vec![255], &storage);
        let table = MerklePatriciaTable::new(map);

        table.put(&vec![230;32], vec![1]).unwrap(); //just to notify the compiler of the types used; same key is added and then removed from tree
        table.delete(&vec![230;32]).unwrap();

        let search_res = table.construct_path_to_key(&vec![244; 32]).unwrap();
        match search_res {
            RootProofNode::Empty => {} 
            _ => assert!(false),
        }
        //TODO:UNCOMMENT
        // let json_repre = serde_json::to_string(&search_res).unwrap();
        // println!("thall: {:?}", json_repre);

        assert_eq!(table.root_hash().unwrap(), search_res.compute_proof_root());
    }

    #[test]
    fn build_proof_in_leaf_tree() {
        let storage = MemoryDB::new();
        let map = MapTable::new(vec![255], &storage);
        let table = MerklePatriciaTable::new(map);
        let root_key = vec![230;32];
        let root_val = vec![2];
        let searched_key = vec![244; 32];

        table.put(&root_key, root_val.clone()).unwrap();
        let table_root = table.root_hash().unwrap();
        let proof_path = table.construct_path_to_key(&searched_key).unwrap();
        assert_eq!(table_root, proof_path.compute_proof_root() );
        //TODO:UNCOMMENT
        // {
        //     let check_res = verify_proof_consistency(&proof_path, &searched_key, table_root)
        //         .unwrap();
        //     assert!(check_res.is_none());
        // }
        // let json_repre = serde_json::to_string(&proof_path).unwrap();
        // println!("{}", json_repre);

        match proof_path {
            RootProofNode::LeafRootExclusive(key, hash_val) => {
                assert_eq!(key.db_key_data, BitSlice::from_bytes(&root_key).to_db_key());
                assert_eq!(hash_val, hash(&root_val));
            }
            _ => assert!(false),
        }

        let proof_path = table.construct_path_to_key(&root_key).unwrap();
        assert_eq!(table_root, proof_path.compute_proof_root() );
        println!("{:?}", bytes_to_hex(&root_key));
        //TODO:UNCOMMENT
        // {
        //     let check_res = verify_proof_consistency(&proof_path, &root_key, table_root).unwrap();
        //     assert_eq!(*check_res.unwrap(), root_val);
        // }
        // let json_repre = serde_json::to_string(&proof_path).unwrap();
        // println!("{}", json_repre);

        match proof_path {
            RootProofNode::LeafRootInclusive(key, val) => {
                assert_eq!(key.db_key_data, BitSlice::from_bytes(&root_key).to_db_key());
                assert_eq!(val, root_val);
            } 
            _ => assert!(false),
        }
    }

    #[test]
    fn fuzz_insert_build_proofs() {
        let data = generate_fully_random_data_keys(100);

        let storage = MemoryDB::new();
        let map = MapTable::new(vec![255], &storage);
        let table = MerklePatriciaTable::new(map);
        for item in &data {
            table.put(&item.0, item.1.clone()).unwrap();
        }

        let table_root_hash = table.root_hash().unwrap();

        for item in &data {
            let proof_path_to_key = table.construct_path_to_key(&item.0).unwrap();
            println!("{:?}", bytes_to_hex(&item.0));
            assert_eq!(proof_path_to_key.compute_proof_root(), table_root_hash);
            //TODO:UNCOMMENT
            // let check_res = verify_proof_consistency(&proof_path_to_key, &item.0, table_root_hash);
            // let proved_value: Option<&Vec<u8>> = check_res.unwrap();
            // assert_eq!(*proved_value.unwrap(), item.1);

            // let json_repre = serde_json::to_string(&proof_path_to_key).unwrap();
            // println!("{}", json_repre);
        }
    }

    #[test]
    fn fuzz_delete_build_proofs() {
        let data = generate_fully_random_data_keys(100);
        let mut rng = rand::thread_rng();

        let storage1 = MemoryDB::new();
        let map1 = MapTable::new(vec![255], &storage1);
        let table1 = MerklePatriciaTable::new(map1);
        for item in &data {
            table1.put(&item.0, item.1.clone()).unwrap();
        }

        let mut keys_to_remove = data.iter()
            .take(50)
            .map(|item| item.0.clone())
            .collect::<Vec<_>>();

        rng.shuffle(&mut keys_to_remove);
        for key in &keys_to_remove {
            table1.delete(key).unwrap();
        }
        let table_root_hash = table1.root_hash().unwrap();
        for key in &keys_to_remove {
            let proof_path_to_key = table1.construct_path_to_key(key).unwrap();
            println!("{:?}", bytes_to_hex(key));
            assert_eq!(proof_path_to_key.compute_proof_root(), table_root_hash);
            //TODO:UNCOMMENT
            // let check_res = verify_proof_consistency(&proof_path_to_key, key, table_root_hash);
            // assert!(check_res.is_ok());
            // let proved_value: Option<&Vec<u8>> = check_res.unwrap();
            // assert!(proved_value.is_none());

            // let json_repre = serde_json::to_string(&proof_path_to_key).unwrap();
            // println!("{}", json_repre);
        }
    }

    #[test]
    fn fuzz_delete() {
        let mut data = generate_random_data(100);
        let mut rng = rand::thread_rng();

        let storage1 = MemoryDB::new();
        let map1 = MapTable::new(vec![255], &storage1);
        let table1 = MerklePatriciaTable::new(map1);
        for item in &data {
            table1.put(&item.0, item.1.clone()).unwrap();
        }

        let storage2 = MemoryDB::new();
        let map2 = MapTable::new(vec![255], &storage2);
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

        for key in &keys_to_remove {
            assert!(table1.get(key).unwrap().is_none());
            assert!(table2.get(key).unwrap().is_none());
        }

        assert!(table2.root_hash().unwrap() != Hash::zero());
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
        let storage1 = MemoryDB::new();
        let map1 = MapTable::new(vec![255], &storage1);
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
