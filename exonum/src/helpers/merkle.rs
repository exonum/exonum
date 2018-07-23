// Copyright 2018 The Exonum Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Merkle tree utilities.

use crypto::Hash;
use storage::proof_list_index::{hash_one, hash_pair};

/// Computes Merkle root hash for a given list of hashes.
///
/// If `hashes` are empty then `Hash::zero()` value is returned.
pub fn root_hash(hashes: &[Hash]) -> Hash {
    if hashes.is_empty() {
        return Hash::zero();
    }
    let mut current_hashes = hashes.to_vec();
    while current_hashes.len() > 1 {
        combine_hash_list(&mut current_hashes);
    }
    current_hashes[0]
}

fn combine_hash_list(hashes: &mut Vec<Hash>) {
    let old_len = hashes.len();
    let new_len = (old_len + 1) / 2;

    for i in 0..old_len / 2 {
        hashes[i] = hash_pair(&hashes[i * 2], &hashes[i * 2 + 1]);
    }
    if old_len % 2 == 1 {
        hashes[new_len - 1] = hash_one(&hashes[old_len - 1]);
    }

    hashes.resize(new_len, Hash::zero());
}

#[cfg(test)]
mod tests {
    use crypto::{self, Hash};
    use storage::{Database, MemoryDB, ProofListIndex};

    use super::*;

    /// Cross-verify `root_hash()` with `ProofListIndex` against expected root hash value.
    fn assert_root_hash_correct(hashes: &[Hash]) {
        let root_actual = root_hash(hashes);
        let root_index = proof_list_index_root(hashes);
        assert_eq!(root_actual, root_index);
    }

    fn proof_list_index_root(hashes: &[Hash]) -> Hash {
        let db = MemoryDB::new();
        let mut fork = db.fork();
        let mut index = ProofListIndex::new("merkle_root", &mut fork);
        index.extend(hashes.iter().cloned());
        index.merkle_root()
    }

    fn hash_list(bytes: &[&[u8]]) -> Vec<Hash> {
        bytes.iter().map(|chunk| crypto::hash(chunk)).collect()
    }

    #[test]
    fn root_hash_single() {
        assert_root_hash_correct(
            &hash_list(&[b"1"]),
        );
    }

    #[test]
    fn root_hash_even() {
        assert_root_hash_correct(
            &hash_list(&[b"1", b"2", b"3", b"4"]),
        );
    }

    #[test]
    fn root_hash_odd() {
        assert_root_hash_correct(
            &hash_list(&[b"1", b"2", b"3", b"4", b"5"]),
        );
    }

    #[test]
    fn root_hash_empty() {
        assert_root_hash_correct(
            &hash_list(&[]),
        );
    }
}
