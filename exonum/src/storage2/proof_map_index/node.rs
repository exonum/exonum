use crypto::{Hash, hash, HASH_SIZE};

use super::super::StorageValue;
use super::key::{ProofMapKey, ChildKind, KEY_SIZE, DB_KEY_SIZE};


const BRANCH_NODE_SIZE: usize = 2 * (HASH_SIZE + DB_KEY_SIZE);

enum Node<T: StorageValue> {
    Leaf(T),
    Branch(BranchNode),
}

// TODO find more padding friendly data layout
// struct BranchData {
//     left_hash:      &Hash,       HASH_SIZE,
//     right_hash:     &Hash,       HASH_SIZE,
//     left_prefix:    &ProofMapKey    DB_KEY_SIZE,
//     right_prefix:   &ProofMapKey    DB_KEY_SIZE
// }
#[derive(Clone)]
struct BranchNode {
    raw: Vec<u8>,
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
    }

    fn child_hash(&self, kind: ChildKind) -> &Hash {
        unsafe {
            let from = match kind {
                ChildKind::Right => HASH_SIZE,
                ChildKind::Left => 0,
            };
            self.read_hash(mem::transmute(&self.raw[from]))
        }
    }

    fn child_slice(&self, kind: ChildKind) -> ProofMapKey {
        let from = match kind {
            ChildKind::Right => 2 * HASH_SIZE + DB_KEY_SIZE,
            ChildKind::Left => 2 * HASH_SIZE,
        };
        ProofMapKey::from_slice(&self.raw[from..from + DB_KEY_SIZE])
    }

    fn set_child_slice(&mut self, kind: ChildKind, prefix: &ProofMapKey) {
        let from = match kind {
            ChildKind::Right => 2 * HASH_SIZE + DB_KEY_SIZE,
            ChildKind::Left => 2 * HASH_SIZE,
        };
        self.write_slice(from, prefix);
    }

    fn set_child_hash(&mut self, kind: ChildKind, hash: &Hash) {
        unsafe {
            let from = match kind {
                ChildKind::Right => HASH_SIZE,
                ChildKind::Left => 0,
            };

            self.raw[from..from + HASH_SIZE].copy_from_slice(hash.as_ref());
            from + HASH_SIZE
        }
    }

    fn set_child(&mut self, kind: ChildKind, prefix: &ProofMapKey, hash: &Hash) {
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

    fn write_slice(&mut self, from: usize, slice: &ProofMapKey) -> usize {
        debug_assert!(slice.data.len() == KEY_SIZE);

        let db_key = slice.to_db_key();

        self.raw[from..from + DB_KEY_SIZE].copy_from_slice(&db_key);
        from + DB_KEY_SIZE
    }
}

// impl StorageValue for BranchNode {
//     fn serialize(self) -> Vec<u8> {
//         self.raw
//     }

//     fn deserialize(v: Vec<u8>) -> Self {
//         BranchNode::from_bytes(v)
//     }

//     fn hash(&self) -> Hash {
//         self.hash()
//     }
// }

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
