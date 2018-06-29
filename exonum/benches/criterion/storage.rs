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

use criterion::{Bencher, Criterion};
use exonum::storage::{
    proof_map_index::PROOF_MAP_KEY_SIZE as KEY_SIZE, Database, DbOptions, ProofListIndex,
    ProofMapIndex, RocksDB,
};
use rand::{Rng, SeedableRng, XorShiftRng};
use tempdir::TempDir;

use std::collections::HashSet;

const NAME: &str = "name";

fn generate_random_kv(len: usize) -> Vec<([u8; KEY_SIZE], Vec<u8>)> {
    let mut rng = XorShiftRng::from_seed([1, 56, 168, 192]);
    let mut exists_keys = HashSet::new();
    let mut base = [0; KEY_SIZE];
    rng.fill_bytes(&mut base);
    let base = base;

    let kv_generator = |_| {
        let mut v = vec![0; 8];

        // Generate only unique keys
        let mut k = base;
        let byte: usize = rng.gen_range(0, 31);
        k[byte] = rng.gen::<u8>();

        rng.fill_bytes(&mut v);
        while exists_keys.contains(&k) {
            rng.fill_bytes(&mut k);
        }
        exists_keys.insert(k);
        (k, v)
    };

    (0..len).map(kv_generator).collect::<Vec<_>>()
}

fn merkle_table_insertion<T: Database>(b: &mut Bencher, db: &T) {
    let mut rng = XorShiftRng::from_seed([192, 168, 56, 1]);
    let mut storage = db.fork();
    let mut table = ProofListIndex::new(NAME, &mut storage);

    b.iter(|| {
        let v_generator = |_| {
            let mut chunk = vec![0; 10];
            rng.fill_bytes(&mut chunk);
            chunk
        };

        for item in (0..1000).map(v_generator) {
            table.push(item);
        }
    });
}

fn merkle_patricia_table_fork_insert<T: Database>(b: &mut Bencher, db: &T) {
    let data = generate_random_kv(200);
    let mut storage = db.fork();
    let mut table = ProofMapIndex::new(NAME, &mut storage);

    b.iter(|| {
        for item in &data {
            table.put(&item.0, item.1.clone());
        }
    });
}

fn merkle_patricia_table_merge_insert<T: Database>(b: &mut Bencher, db: &T) {
    let data = generate_random_kv(200);

    b.iter(|| {
        let patch;
        {
            let mut fork = db.fork();
            {
                let mut table = ProofMapIndex::new(NAME, &mut fork);
                for item in &data {
                    table.put(&item.0, item.1.clone());
                }
            }
            patch = fork.into_patch();
        }
        db.merge(patch).unwrap();
    });
}

fn create_rocksdb(tempdir: &TempDir) -> RocksDB {
    let options = DbOptions::default();
    RocksDB::open(tempdir.path(), &options).unwrap()
}

fn bench_merkle_table_append_rocksdb(b: &mut Bencher) {
    let tempdir = TempDir::new("exonum").unwrap();
    let db = create_rocksdb(&tempdir);
    merkle_table_insertion(b, &db);
}

fn bench_merkle_patricia_table_insertion_merge_rocksdb(b: &mut Bencher) {
    let tempdir = TempDir::new("exonum").unwrap();
    let db = create_rocksdb(&tempdir);
    merkle_patricia_table_merge_insert(b, &db);
}

fn bench_merkle_patricia_table_insertion_fork_rocksdb(b: &mut Bencher) {
    let tempdir = TempDir::new("exonum").unwrap();
    let db = create_rocksdb(&tempdir);
    merkle_patricia_table_fork_insert(b, &db);
}

pub fn bench_storage(c: &mut Criterion) {
    ::exonum::crypto::init();

    c.bench_function("append merkle table", bench_merkle_table_append_rocksdb);
    c.bench_function(
        "insert without merge merkle table",
        bench_merkle_patricia_table_insertion_fork_rocksdb,
    );
    c.bench_function(
        "insert with merge merkle table",
        bench_merkle_patricia_table_insertion_merge_rocksdb,
    );
}
