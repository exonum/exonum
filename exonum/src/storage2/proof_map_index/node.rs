use crypto::{Hash, hash, HASH_SIZE};

use super::super::{StorageKey, StorageValue};
use super::key::{ProofMapKey, ChildKind, KEY_SIZE, DB_KEY_SIZE};

// TODO: implement Field for ProofMapKey and define BranchNode as StorageValue

const BRANCH_NODE_SIZE: usize = 2 * (HASH_SIZE + DB_KEY_SIZE);

enum Node<T: StorageValue> {
    Leaf(T),
    Branch(BranchNode),
}

#[derive(Clone)]
struct BranchNode {
    raw: Vec<u8>,
}

impl BranchNode {
    fn empty() -> BranchNode {
        BranchNode { raw: vec![0; BRANCH_NODE_SIZE] }
    }

    fn child_hash(&self, kind: ChildKind) -> &Hash {
        unsafe {
            let from = match kind {
                ChildKind::Right => HASH_SIZE,
                ChildKind::Left => 0,
            };
            ::std::mem::transmute(&self.raw[from])
        }
    }

    fn child_slice(&self, kind: ChildKind) -> ProofMapKey {
        let from = match kind {
            ChildKind::Right => 2 * HASH_SIZE + DB_KEY_SIZE,
            ChildKind::Left => 2 * HASH_SIZE,
        };
        ProofMapKey::read(&self.raw[from..from + DB_KEY_SIZE])
    }

    fn set_child_slice(&mut self, kind: ChildKind, prefix: &ProofMapKey) {
        let from = match kind {
            ChildKind::Right => 2 * HASH_SIZE + DB_KEY_SIZE,
            ChildKind::Left => 2 * HASH_SIZE,
        };
        prefix.write(&mut self.raw[from..from + DB_KEY_SIZE]);
    }

    fn set_child_hash(&mut self, kind: ChildKind, hash: &Hash) {
        unsafe {
            let from = match kind {
                ChildKind::Right => HASH_SIZE,
                ChildKind::Left => 0,
            };
            self.raw[from..from + HASH_SIZE].copy_from_slice(hash.as_ref());
        }
    }

    fn set_child(&mut self, kind: ChildKind, prefix: &ProofMapKey, hash: &Hash) {
        self.set_child_slice(kind, prefix);
        self.set_child_hash(kind, hash);
    }
}

impl StorageValue for BranchNode {
    fn into_vec(self) -> Vec<u8> {
        self.raw
    }

    fn from_slice(value: &[u8]) -> Self {
        BranchNode { raw: value.to_vec() }
    }

    fn from_vec(value: Vec<u8>) -> Self {
        BranchNode { raw: value }
    }

    fn hash(&self) -> Hash {
        hash(&self.raw)
    }
}

// impl ::std::fmt::Debug for BranchNode {
//     fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
//         f.debug_struct("BranchNode")
//             .field("left_prefix", &self.child_slice(ChildKind::Left))
//             .field("left_hash", &bytes_to_hex(self.child_hash(ChildKind::Left)))
//             .field("right_prefix", &self.child_slice(ChildKind::Right))
//             .field("right_hash",
//                    &bytes_to_hex(self.child_hash(ChildKind::Right)))
//             .field("hash", &bytes_to_hex(&self.hash()))
//             .finish()
//     }
// }
