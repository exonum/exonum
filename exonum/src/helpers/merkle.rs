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

use crypto::{self, Hash, HashStream};

/// Computes Merkle root hash for a given list of hashes.
///
/// If `hashes` are empty then `Hash::zero()` value is returned.
pub fn root_hash<'a, I: IntoIterator<Item = &'a Hash>>(hashes: I) -> Hash {
    let mut current_hashes: Vec<Hash> = hashes.into_iter().cloned().collect();
    if current_hashes.is_empty() {
        return Hash::zero();
    }
    while current_hashes.len() > 1 {
        current_hashes = combine_hash_list(&current_hashes);
    }
    current_hashes[0]
}

fn combine_hash_list(hashes: &[Hash]) -> Vec<Hash> {
    hashes
        .chunks(2)
        .map(|pair|
            // Keep hash combination consistent with ProofListIndex.
            if pair.len() == 2 {
                hash_pair(&pair[0], &pair[1])
            } else {
                hash_one(&pair[0])
            }
        )
        .collect()
}

fn hash_one(lhs: &Hash) -> Hash {
    crypto::hash(lhs.as_ref())
}

fn hash_pair(lhs: &Hash, rhs: &Hash) -> Hash {
    HashStream::new()
        .update(lhs.as_ref())
        .update(rhs.as_ref())
        .hash()
}

#[cfg(test)]
mod tests {
    use crypto::{self, Hash};
    use encoding::serialize::FromHex;
    use storage::{Database, MemoryDB, ProofListIndex};

    use super::*;

    /// Cross-verify `root_hash()` with `ProofListIndex` against expected root hash value.
    fn assert_root_hash_eq(hashes: &[Hash], expected: &str) {
        let root_actual = root_hash(hashes);
        let root_index = proof_list_index_root(hashes);
        let root_expected = Hash::from_hex(expected).expect("hex hash");
        assert_eq!(root_actual, root_expected);
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
        assert_root_hash_eq(
            &hash_list(&[b"1"]),
            "6b86b273ff34fce19d6b804eff5a3f5747ada4eaa22f1d49c01e52ddb7875b4b",
        );
    }

    #[test]
    fn root_hash_even() {
        assert_root_hash_eq(
            &hash_list(&[b"1", b"2", b"3", b"4"]),
            "cd53a2ce68e6476c29512ea53c395c7f5d8fbcb4614d89298db14e2a5bdb5456",
        );
    }

    #[test]
    fn root_hash_odd() {
        assert_root_hash_eq(
            &hash_list(&[b"1", b"2", b"3", b"4", b"5"]),
            "9d6f6f12f390c2f281beacc79fd527f2355f555aa6f47682de41cbaf7756e187",
        );
    }

    #[test]
    fn root_hash_empty() {
        assert_root_hash_eq(
            &hash_list(&[]),
            "0000000000000000000000000000000000000000000000000000000000000000",
        );
    }
}
