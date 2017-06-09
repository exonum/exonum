use std::mem;
use std::cmp::{min, PartialEq};
use std::marker::PhantomData;
use std::fmt;
use std::ops::Not;

use crypto::{hash, Hash, HASH_SIZE};

use super::utils::bytes_to_hex;
use super::base_table::BaseTable;
use super::{Map, Error, View, StorageKey, StorageValue};

pub use self::proofpathtokey::{RootProofNode, BranchProofNode, ProofNode, BitVec};

pub mod proofpathtokey;


type Entry<V> = (Vec<u8>, Node<V>);

pub struct MerklePatriciaTable<'a, K, V> {
    base: BaseTable<'a>,
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
impl<'a, K: StorageKey, V: StorageValue> MerklePatriciaTable<'a, K, V> {

    pub fn root_hash(&self) -> Hash {
        match self.root_node()? {
            Some((root_db_key, Node::Leaf(value))) => {
                Ok(hash(&[root_db_key.as_slice(), value.hash().as_ref()].concat()))
            }
            Some((_, Node::Branch(branch))) => Ok(branch.hash()),
            None => Ok(Hash::zero()),
        }
    }

    fn root_node(&self) -> Option<Entry<V>> {
        let out = match self.root_prefix()? {
            Some(db_key) => {
                let node = self.get_node_unchecked(&db_key)?;
                Some((db_key, node))
            }
            None => None,
        };
        Ok(out)
    }

    fn insert(&self, key: &[u8], value: V) -> () {
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
                        -> (Option<usize>, Hash) {
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
                match self.get_node_unchecked(child_slice.to_db_key())? {
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

    fn remove(&self, key_slice: BitSlice) -> () {
        match self.root_node()? {
            // If we have only on leaf, then we just need to remove it (if any)
            Some((prefix, Node::Leaf(_))) => {
                let key = key_slice.to_db_key();
                if key == prefix {
                    self.base.delete(&key)?;
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
                            self.base.delete(&prefix)?;
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
                      -> RemoveResult {
        let mut child_slice = parent.child_slice(key_slice.at(0));
        child_slice.from = key_slice.from;
        let i = child_slice.common_prefix(key_slice);

        if i == child_slice.len() {
            match self.get_node_unchecked(child_slice.to_db_key())? {
                Node::Leaf(_) => {
                    self.base.delete(&key_slice.to_db_key())?;
                    return Ok(RemoveResult::Leaf);
                }
                Node::Branch(mut branch) => {
                    let suffix_slice = key_slice.mid(i);
                    match self.do_remove_node(&branch, &suffix_slice)? {
                        RemoveResult::Leaf => {
                            let child = !suffix_slice.at(0);
                            let key = branch.child_slice(child).to_db_key();
                            let hash = branch.child_hash(child);

                            self.base.delete(&child_slice.to_db_key())?;

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

    pub fn construct_path_to_key(&self, searched_key: &[u8]) -> RootProofNode<V> {
        debug_assert_eq!(searched_key.len(), KEY_SIZE);
        let searched_slice = BitSlice::from_bytes(searched_key);
        let suff_from = 0;

        let res: RootProofNode<V> = match self.root_node()? {
            Some((root_db_key, Node::Leaf(root_value))) => {
                if searched_slice.to_db_key() == root_db_key {
                    RootProofNode::LeafRootInclusive(BitVec::new(root_db_key,
                                                                 suff_from,
                                                                 (KEY_SIZE * 8) as u16),
                                                     root_value)
                } else {
                    RootProofNode::LeafRootExclusive(BitVec::new(root_db_key,
                                                                 suff_from,
                                                                 (KEY_SIZE * 8) as u16),
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
                                    left_key: BitVec::new(l_s_db_key, suff_from, l_s.to),
                                    right_key: BitVec::new(r_s_db_key, suff_from, r_s.to),
                                })
                            }
                            ChildKind::Right => {
                                RootProofNode::Branch(BranchProofNode::RightBranch {
                                    left_hash: neighbour_child_hash,
                                    right_hash: Box::new(child_proof),
                                    left_key: BitVec::new(l_s_db_key, suff_from, l_s.to),
                                    right_key: BitVec::new(r_s_db_key, suff_from, r_s.to),
                                })
                            }
                        }
                    } else {
                        let l_h = *branch.child_hash(ChildKind::Left); //copy
                        let r_h = *branch.child_hash(ChildKind::Right);//copy
                        RootProofNode::Branch(BranchProofNode::BranchKeyNotFound {
                            left_hash: l_h,
                            right_hash: r_h,
                            left_key: BitVec::new(l_s_db_key, suff_from, l_s.to),
                            right_key: BitVec::new(r_s_db_key, suff_from, r_s.to),
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
                        left_key: BitVec::new(l_s_db_key, suff_from, l_s.to),
                        right_key: BitVec::new(r_s_db_key, suff_from, r_s.to),
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
                                       -> Option<ProofNode<V>> {

        let mut child_slice = current_branch.child_slice(searched_slice.at(0));
        child_slice.from = searched_slice.from;
        let c_pr_l = child_slice.common_prefix(searched_slice);
        let suff_from = searched_slice.from + c_pr_l as u16;
        debug_assert!(c_pr_l > 0);
        if c_pr_l < child_slice.len() {
            return Ok(None);
        }

        let res: ProofNode<V> = match self.get_node_unchecked(child_slice.to_db_key())? {
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
                                left_key: BitVec::new(l_s_db_key, suff_from, l_s.to),
                                right_key: BitVec::new(r_s_db_key, suff_from, r_s.to),
                            })
                        }
                        ChildKind::Right => {
                            ProofNode::Branch(BranchProofNode::RightBranch {
                                left_hash: neighbour_child_hash,
                                right_hash: Box::new(child_proof),
                                left_key: BitVec::new(l_s_db_key, suff_from, l_s.to),
                                right_key: BitVec::new(r_s_db_key, suff_from, r_s.to),
                            })
                        }
                    }
                } else {
                    let l_h = *child_branch.child_hash(ChildKind::Left); //copy
                    let r_h = *child_branch.child_hash(ChildKind::Right);//copy
                    ProofNode::Branch(BranchProofNode::BranchKeyNotFound {
                        left_hash: l_h,
                        right_hash: r_h,
                        left_key: BitVec::new(l_s_db_key, suff_from, l_s.to),
                        right_key: BitVec::new(r_s_db_key, suff_from, r_s.to),
                    })
                    // proof of exclusion of a key, because none of child slices is a prefix(searched_slice)
                }
            }
        };
        Ok(Some(res))
    }

    fn insert_leaf(&self, key: &BitSlice, value: V) -> Hash {
        debug_assert!(key.is_leaf_key());

        let hash = value.hash();
        let db_key = key.to_db_key();
        let bytes = value.serialize();
        self.base.put(&db_key, bytes)?;
        Ok(hash)
    }

    fn insert_branch(&self, key: &BitSlice, branch: BranchNode) -> () {
        let db_key = key.to_db_key();
        self.base.put(&db_key, branch.serialize())
    }
}
